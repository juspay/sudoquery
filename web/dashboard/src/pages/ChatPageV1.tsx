import React, { useEffect, useState, useRef, type ReactNode } from 'react';
import { Box } from '@mui/material';
import { apiClient } from '../services/apiClient';
import { useProject } from '../contexts/ProjectContext';
import { ChatSidebar } from '../components/Chat/ChatSidebar';
import { ChatInterface, type ChatInterfaceMessage } from '../components/Chat/ChatInterface';
import { DateTimePicker } from '../components/Chat/DateTimePicker';
import { TypeChangeConfirmDialog } from '../components/Chat/TypeChangeConfirmDialog';
import { generateUUID } from '../utils/uuid';
import type { Chat, ChatType } from '../types/chat';
import { UserMessageBubble } from '../components/Chat/UserMessageBubble';
import { AssistantMessageBubble } from '../components/Chat/AssistantMessageBubble';
import { renderToolCall } from '../components/Chat/toolCallRenderer';
import { CircularLoader } from '../components/shared/CircularLoader';
import { useNavigate, useSearchParams } from 'react-router-dom';
/*
 * this page contains chat history on sidebar and a chat window on the right.
 * user can click on the chat history to load the chat in the chat window, or user can click on the new chat button to start a new chat.
 * initial chat will be choosen based on url. if url contains chatId, then we will load the chat with the given chatId, otherwise we will start a new chat.
 * Chat page where some of toolcalls are auto handling in server.
 * */

// chatId can be null or undefined, if it's null or undefined, it means this is a new chat, and we need to create a new chat when user send the first message. otherwise, we can just load the chat with the given chatId.
export default function ChatPageV1() {
  const project = useProject();
  if (project == null) {
    return <p>Loading project...</p>;
  }
  return <Page project={project.currentProject} />;
}

function Page({project}: {project : string}){
  const url = new URL(window.location.href);
  const pathSegments = url.pathname.split('/');
  const chatIdFromUrl = pathSegments.length > 3 ? pathSegments[3] : null;

  const getChatType = ()=>{
    const url = new URL(window.location.href);
    const urlChatType = url.searchParams.get('chat_type') as ChatType | null;
    return urlChatType === 'create_dashboard' ? 'create_dashboard' : 'general';
  }

  const [chats, setChats] = React.useState<Chat[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(chatIdFromUrl);
  const [chatInstanceKey, setChatInstanceKey] = useState(chatIdFromUrl ?? `new-chat-${getChatType()}`);

  const updateUrlForChat = (nextChatId: string | null) => {
    const nextUrl = new URL(window.location.href);
    const nextPathSegments = nextUrl.pathname.split('/');
    const baseSegments = nextPathSegments.slice(0, 3);

    nextUrl.pathname = nextChatId
      ? [...baseSegments, nextChatId].join('/')
      : baseSegments.join('/');

    window.history.replaceState({}, '', nextUrl.toString());
  };

  useEffect(() => {
    apiClient.request<{ chats: Chat[] }>('/project/chats')
      .then((data) => {
        setChats(data ?? []);
      })
      .catch((error) => {
        console.error('Failed to fetch chats from server:', error);
      });
  }, [project]);

  const handleSelectChat = (id: string) => {
    setActiveChatId(id);
    setChatInstanceKey(id);
    updateUrlForChat(id);
  };

  const handleNewChat = () => {
    setActiveChatId(null);
    setChatInstanceKey(`new-chat-{getChatType()}`);
    updateUrlForChat(null);
  }

  const [searchParams, setSearchParams] = useSearchParams()

  const handleChatIdReceived = (value: React.SetStateAction<string | null>) => {
    setActiveChatId((currentChatId) => {
      const nextChatId = typeof value === 'function' ? value(currentChatId) : value;
      updateUrlForChat(nextChatId);
      return nextChatId;
    });
    setChats((c)=>{
      return [{id: value, title: "New Chat", createdAt: Date.now(), chat_type: searchParams.get("chat_type")}, ...c]
    });
  };

  const handleDeleteChat = async (chatId: string) => {
    try {
      await apiClient.delete(`/project/chat/${chatId}`);

      // Remove chat from local state
      setChats((prevChats) => prevChats.filter((c) => c.id !== chatId));

      // If the deleted chat was active, reset to new chat
      if (activeChatId === chatId) {
        handleNewChat();
      }
    } catch (error) {
      console.error('Failed to delete chat:', error);
    }
  };

  const handleTitleGenerated = (generatedChatId: string, title: string) => {
    setChats((prevChats) =>
      prevChats.map((c) =>
        c.id === generatedChatId ? { ...c, title } : c
      )
    );
  };

  return (
    <Box sx={{ display: 'flex', flexDirection: 'row', height: '100%', overflow: 'hidden' }}>
      <ChatSidebar
        chats={chats}
        activeChatId={activeChatId}
        onSelectChat={handleSelectChat}
        onNewChat={handleNewChat}
        onDeleteChat={handleDeleteChat}
      />
      <Box sx={{ flex: 1, display: 'flex', flexDirection: 'row', height: '100%', overflow: 'hidden', minWidth: 0, minHeight: 0 }}>
        <Chat
          key={chatInstanceKey}
          chatId={activeChatId}
          chats={chats}
          onChatIdReceived={handleChatIdReceived}
          onTitleGenerated={handleTitleGenerated}
          onNewChat={handleNewChat}
        />
      </Box>
    </Box>
  );
}


export interface UserMessage {
  id: string;
  type: 'user_message';
  content: string;
}

export interface PlainLlmMessage {
  id: string;
  type: 'plain_llm_message';
  content: string;
}

export interface ToolCallMessage {
  id: string;
  type: 'tool_call_message';
  content: {"function": {arguments: string, name: string}, id: string};
}

export type ChatInterfaceMessage =
  | UserMessage
  | PlainLlmMessage
  | ToolCallMessage;

interface ChatMessageRecord {
  id: string;
  chat_id: string;
  created_at: string;
  isComplete?: boolean;
  message: {
    role?: 'user' | 'assistant' | 'tool';
    content?: string | null;
    tool_calls?: Array<{
      id: string;
      index: number,
      function: {
        name: string;
        arguments: string;
      };
    }>;
    tool_call_id?: string;
  };
}

function mapServerMessagesToChatInterfaceMessages(
  records: ChatMessageRecord[],
  handleSendMessage,
  isStreaming: boolean,
  chatId: string | null
): ReactNode[][] {
  const groups: ReactNode[][] = [];
  let runningGroup: ReactNode[] = [];

  for(let i=0; i<records.length; i++){
    const record = records[i];
    if(record.message.role === 'user'){
      if(runningGroup.length > 0){
        groups.push(runningGroup);
        runningGroup = [];
      }
      runningGroup.push(<UserMessageBubble
        content={record.message.content ?? ''}
        onRetry={()=>handleSendMessage(record.message.content, undefined, record.id)}
        />)
    } else if(record.message.role === 'assistant'){
      if(record?.message?.content && record.message.content.trim() != ''){
         runningGroup.push(<AssistantMessageBubble content={record.message.content ?? ''}/>);
      }
      if(record.message.tool_calls){
        for(const tool_call of record.message.tool_calls){
          const toolNode = renderToolCall(tool_call, records, i, record.isComplete, chatId ?? undefined);
          if (toolNode) {
            runningGroup.push(toolNode);
          }
        }
      }
    }
  }

  if(runningGroup.length > 0){
    if(isStreaming){
      runningGroup.push(<CircularLoader size={15} thickness={2}/>);
    }
    groups.push(runningGroup);
  }
  return groups;
}

function useChatMessages({
  chatId,
  chatType,
  onChatIdReceived,
  onTitleGenerated,
}: {
  chatId: null | string;
  chatType: string;
  onChatIdReceived: React.Dispatch<React.SetStateAction<string | null>>;
  onTitleGenerated?: (chatId: string, title: string) => void;
}) {
  const [rawMessages, setRawMessages] = useState<ChatMessageRecord[]>([]);
  const [isStreaming, setIsStreaming] = useState(false);

  // get initial messages if chatId is not null, otherwise start with empty messages for new chat
  useEffect(() => {
    if (chatId) {
      apiClient.request<{ messages: ChatMessageRecord[] }>(`/project/chat/${chatId}/messages`)
        .then((data) => {
          if(!data.messages) return;
          setRawMessages(data.messages.map((msg)=>{return {...msg, isComplete: true}}) ?? []);
        })
        .catch((error) => {
          console.error('Failed to fetch chat messages from server:', error);
        });
    } else {
      setRawMessages([]);
    }
  }, [chatId])

  // title generation api call
  useEffect(()=>{

  }, [chatId])

  function generateTitle(chatId: string | null){
    if(chatId !== null){
        apiClient.post<{ title: string }>('/project/chat/gen_titlev1', { chat_id: chatId })
          .then((response) => {
            if (onTitleGenerated) {
              onTitleGenerated(chatId, response.title);
            }
          })
          .catch((err) => {
            console.error('Failed to generate title:', err);
          });
      }
  }


  const handleSendMessage = async (content: string, messageType?: ChatType, message_id?: string) => {
    const userMessageId = message_id ?? '';
    if(message_id == null || message_id == undefined){
      setRawMessages((prev) => [
        ...prev,
        {
          id: userMessageId,
          chat_id: chatId ?? '',
          created_at: new Date().toISOString(),
          message: {
            role: 'user',
            content,
          },
        },
      ]);
    } else {
      setRawMessages((prev)=>{
        const msg_index = prev.findIndex(msg => msg.id == message_id);
        if(msg_index > -1){
          return prev.slice(0, msg_index+1)
        }
        return prev;
      })
    }

    setIsStreaming(true);

    // get streaming response messages from server
    // message type will be choices: [{delta: {content: string, tool_calls: [{function: {arguments: string, name: string}, id: string}]}, finish_reason: string, index: number}]
    // stop listening when there is connection close
    // Use the type from the message if provided (for new chats), otherwise use the chat's stored type
    const effectiveChatType = messageType ?? chatType;
    const response = await apiClient.stream('/project/chat/llm_chatv1', { prompt: content, role: 'user', chat_id: chatId, message_id: message_id, chat_type: effectiveChatType });

    if (!response.body) {
      throw new Error('Streaming response body is missing');
    }

    // call generate title
    generateTitle(chatId);

    const reader = response.body.getReader();

    const decoder = new TextDecoder();
    let done = false;
    let buffer = '';

    const processEvent = (rawEvent: string) => {
      const eventType = rawEvent
        .split('\n')
        .map((line) => line.trim())
        .find((line) => line.startsWith('event:'))
        ?.slice(6)
        .trim();

      const dataLines = rawEvent
        .split('\n')
        .map((line) => line.trim())
        .filter((line) => line.startsWith('data:'))
        .map((line) => line.slice(5).trim());

      if (eventType === 'message_end') {
        if (dataLines.length === 0) {
          setRawMessages((msgs)=> msgs.map((m, i)=> i == msgs.length - 1 ? {...m, isComplete: true} : m))
          console.log("for message_end, didn't receive any datalines")
          return;
        }
        console.log("received message_end")
        // we receive message id {"message_id": saved_msg.id.to_string()}
        setRawMessages((msgs)=> msgs.map((m, i)=> i == msgs.length - 1 ? {...m, isComplete: true, id: JSON.parse(dataLines.join('\n')).message_id} : m))
        return;
      }

      // type used for complete messages instead of delta (primarily for searver tool call responses)
      if(eventType === "complete_message"){
        if(dataLines.length != 0){
          setRawMessages((msgs)=> {
            return [...msgs, {message: JSON.parse(dataLines.join('\n')), chat_id: chatId}]
          })
        }
        return;
      }

      if (dataLines.length === 0) {
        return;
      }

      const dataText = dataLines.join('\n');

      if (eventType === 'chat_id') {
        if (!chatId && dataText) {
          onChatIdReceived(dataText);
          generateTitle(dataText)
        }
        return;
      }

      const parsed = JSON.parse(dataText) as {
        choices?: Array<{
          delta?: {
            content?: string;
            tool_calls?: Array<{ function: { arguments: string; name: string }; id: string }>;
          };
        }>;
      };
      const delta = parsed.choices?.[0]?.delta;

      if (delta?.content || delta?.tool_calls) {
        // if last message is completed create a new one
        setRawMessages((prevRawMessages) => {
          if(prevRawMessages[prevRawMessages.length - 1].isComplete){
            return [...prevRawMessages, {message:{}} as ChatMessageRecord];
          }
          return prevRawMessages;
        });
        setRawMessages((prevRawMessages) => {
          // if last message is completed create a new one
          setRawMessages((prevRawMessages) => {
            if(prevRawMessages[prevRawMessages.length - 1].isComplete){
              return [...prevRawMessages, {message:{}} as ChatMessageRecord];
            }
            return prevRawMessages;
          });
          const lastIndex = prevRawMessages.length - 1;
          if (lastIndex >= 0 && prevRawMessages[lastIndex].message.role === 'assistant') {
            return prevRawMessages.map((msg, index) => {
              if (index === lastIndex) {
                return {
                  ...msg,
                  message: deepMergeMsgAndDelta(msg.message, delta),
                };
              }
              return msg;
            });
          } else {
            return [
              ...prevRawMessages,
              {
                id: generateUUID(),
                chat_id: chatId ?? '',
                created_at: new Date().toISOString(),
                message: {
                  role: 'assistant',
                  content: delta.content ?? null,
                  tool_calls: delta.tool_calls,
                },
              },
            ];
          }
        });
      }
    };

    while (!done) {
      const { value, done: doneReading } = await reader.read();
      done = doneReading;
      buffer += decoder.decode(value, { stream: !doneReading });

      const eventChunks = buffer.split('\n\n');
      buffer = eventChunks.pop() ?? '';

      eventChunks.forEach((eventChunk) => {
        try {
          processEvent(eventChunk);
        } catch (error) {
          console.error('Failed to parse stream event', error);
        }
      });
    }

    if (buffer.trim()) {
      try {
        processEvent(buffer);
      } catch (error) {
        console.error('Failed to parse final stream event', error);
      }
    }

    setIsStreaming(false);
  };

  // Derive UI messages from raw server messages
  const groups = mapServerMessagesToChatInterfaceMessages(rawMessages, handleSendMessage, isStreaming, chatId);

  const handleSendToolResponse = async (toolCallId: string, value: unknown) => {
    const toolMessageId = generateUUID();

    // Insert tool response message into raw messages
    setRawMessages((prev) => [
      ...prev,
      {
        id: toolMessageId,
        chat_id: chatId ?? '',
        created_at: new Date().toISOString(),
        message: {
          role: 'tool',
          content: JSON.stringify(value),
          tool_call_id: toolCallId,
        },
      },
    ]);

    setIsStreaming(true);

    // Call the tool response endpoint
    const response = await apiClient.stream('/project/chat/client_tool_response_v1', {
      chat_id: chatId,
      tool_call_id: toolCallId,
      value,
    });

    if (!response.body) {
      throw new Error('Streaming response body is missing');
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let done = false;
    let buffer = '';

    const processEvent = (rawEvent: string) => {
      const eventType = rawEvent
        .split('\n')
        .map((line) => line.trim())
        .find((line) => line.startsWith('event:'))
        ?.slice(6)
        .trim();

      const dataLines = rawEvent
        .split('\n')
        .map((line) => line.trim())
        .filter((line) => line.startsWith('data:'))
        .map((line) => line.slice(5).trim());

      if (eventType === 'message_end') {
        if (dataLines.length === 0) {
          setRawMessages((msgs)=> msgs.map((m, i)=> i == msgs.length - 1 ? {...m, isComplete: true} : m))
          console.log("for message_end, didn't receive any datalines")
          return;
        }
        // we receive message id {"message_id": saved_msg.id.to_string()}
        setRawMessages((msgs)=> msgs.map((m, i)=> i == msgs.length - 1 ? {...m, isComplete: true, id: JSON.parse(dataLines.join('\n')).message_id} : m))
        return;
      }

      // type used for complete messages instead of delta (primarily for searver tool call responses)
      if(eventType === "complete_message"){
        if(dataLines.length != 0){
          setRawMessages((msgs)=> {
            return [...msgs, {message: JSON.parse(dataLines.join('\n')), chat_id: chatId}]
          })
        }
        return;
      }

      if (dataLines.length === 0) {
        return;
      }

      const dataText = dataLines.join('\n');

      if (eventType === 'chat_id') {
        if (!chatId && dataText) {
          onChatIdReceived(dataText);
        }
        return;
      }

      const parsed = JSON.parse(dataText) as {
        choices?: Array<{
          delta?: {
            content?: string;
            tool_calls?: Array<{ function: { arguments: string; name: string }; id: string }>;
          };
        }>;
      };
      const delta = parsed.choices?.[0]?.delta;

      if (delta?.content || delta?.tool_calls) {
        // if last message is completed create a new one
        setRawMessages((prevRawMessages) => {
          if(prevRawMessages[prevRawMessages.length - 1].isComplete){
            return [...prevRawMessages, {message:{}} as ChatMessageRecord];
          }
          return prevRawMessages;
        });
        setRawMessages((prevRawMessages) => {
          const lastIndex = prevRawMessages.length - 1;
          if (lastIndex >= 0 && prevRawMessages[lastIndex].message.role === 'assistant') {
            return prevRawMessages.map((msg, index) => {
              if (index === lastIndex) {
                return {
                  ...msg,
                  message: deepMergeMsgAndDelta(msg.message, delta),
                };
              }
              return msg;
            });
          } else {
            return [
              ...prevRawMessages,
              {
                id: generateUUID(),
                chat_id: chatId ?? '',
                created_at: new Date().toISOString(),
                message: {
                  role: 'assistant',
                  content: delta.content ?? null,
                  tool_calls: delta.tool_calls,
                },
              },
            ];
          }
        });
      }
    };

    while (!done) {
      const { value: chunkValue, done: doneReading } = await reader.read();
      done = doneReading;
      buffer += decoder.decode(chunkValue, { stream: !doneReading });

      const eventChunks = buffer.split('\n\n');
      buffer = eventChunks.pop() ?? '';

      eventChunks.forEach((eventChunk) => {
        try {
          processEvent(eventChunk);
        } catch (error) {
          console.error('Failed to parse stream event', error);
        }
      });
    }

    if (buffer.trim()) {
      try {
        processEvent(buffer);
      } catch (error) {
        console.error('Failed to parse final stream event', error);
      }
    }

    setIsStreaming(false);
  };

  return {
    groups,
    rawMessages,
    isStreaming,
    handleSendMessage,
    handleSendToolResponse,
  };
}

function deepMergeMsgAndDelta(
  message: ChatMessageRecord['message'],
  delta: { content?: string; tool_calls?: Array<{ function: { arguments: string; name: string }; id: string, index: number }> }
): ChatMessageRecord['message'] {
  const newContent = delta.content
    ? (message.content ?? '') + delta.content
    : message.content;

  const newToolCalls = delta.tool_calls
    ? mergeToolCalls(message.tool_calls ?? [], delta.tool_calls)
    : message.tool_calls;

  return {
    ...message,
    content: newContent,
    tool_calls: newToolCalls,
  };
}

function mergeToolCalls(
  existing: Array<{ id: string; function: { name: string; arguments: string }, index: number }>,
  incoming: Array<{ id: string; function: { arguments: string; name: string }, index: number }>
): Array<{ id: string; function: { name: string; arguments: string } }> {
  const result = [...existing];

  for (const incomingCall of incoming) {
    const existingIndex = result.findIndex((call) => call.index === incomingCall.index);

    if (existingIndex >= 0) {
      result[existingIndex] = {
        ...result[existingIndex],
        function: {
          ...result[existingIndex].function,
          arguments: result[existingIndex].function.arguments + incomingCall.function.arguments,
        },
      };
    } else {
      result.push(incomingCall);
    }
  }

  return result;
}

function Chat({
  chatId,
  chats,
  onChatIdReceived,
  onTitleGenerated,
  onNewChat
}: {
  chatId: null | string;
  chats: Chat[];
  onChatIdReceived: React.Dispatch<React.SetStateAction<string | null>>;
  onTitleGenerated: (chatId: string, title: string) => void;
  onNewChat: ()=>void;
}) {

  const activeChat = chats.find(c => c.id === chatId);
  const [searchParams, setSearchParams] = useSearchParams()
  const chatType = activeChat?.chat_type ?? searchParams.get("chat_type") as ChatType ?? "general";
  const { groups, rawMessages, handleSendMessage, handleSendToolResponse } = useChatMessages({ chatId, chatType, onChatIdReceived, onTitleGenerated });

  const [isTypeChangeDialogOpen, setIsTypeChangeDialogOpen] = useState(false);
  const [desiredType, setDesiredType] = useState<ChatType>('general');

  const updateChatTypeInUrl =  (updatedChatType: ChatType) => {
    setSearchParams((prev)=>{
      prev.set("chat_type", updatedChatType);
      return prev;
    })
  }

  const handleTypeChangeAttempt = (newType: ChatType): boolean => {
    if (activeChat) {
      // Existing chat - show confirmation, return false to prevent immediate update
      setDesiredType(newType);
      setIsTypeChangeDialogOpen(true);
      return false;
    } else {
      updateChatTypeInUrl(newType)
      // New chat - allow change directly
      return true;
    }
  };

  const navigate = useNavigate()

  const handleConfirmTypeChange = (updatedChatType: ChatType) => {
    setIsTypeChangeDialogOpen(false);
    updateChatTypeInUrl(updatedChatType);
    //todo: open url with chat_type
    navigate(`/app/chat?chat_type=${updatedChatType}`);
    onNewChat()
  };

  // Find pending request_datetime_range tool calls from raw messages
  const pendingDateRangeCall = rawMessages[rawMessages.length - 1]?.message.tool_calls?.find(
    (call) => call.function.name === 'request_datetime_range');

  return (
      <>
        <ChatInterface
          groups={groups}
          onSendMessage={handleSendMessage}
          placeholder="Ask a question about your data…"
          chatType={chatType}
          onChatTypeChange={handleTypeChangeAttempt}
        />
        {pendingDateRangeCall && (
          <DateTimePicker
            message="Select a date range to filter the data"
            type="range"
            onSubmit={(startDate, endDate) => {
              handleSendToolResponse(pendingDateRangeCall.id, {
                selected_range: {
                  start: startDate,
                  end: endDate,
                },
              });
            }}
            onCancel={() => {
              handleSendToolResponse(pendingDateRangeCall.id, {
                selected_range: null,
                reason: 'user_cancelled',
              });
            }}
          />
        )}
        <TypeChangeConfirmDialog
          isOpen={isTypeChangeDialogOpen}
          fromType={chatType}
          toType={desiredType}
          onConfirm={handleConfirmTypeChange}
          onCancel={() => setIsTypeChangeDialogOpen(false)}
        />
      </>
  );
}
