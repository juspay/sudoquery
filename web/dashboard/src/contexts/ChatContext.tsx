import { createContext, useContext, useState, useMemo, useEffect, useCallback } from 'react';
import type { ReactNode } from 'react';
import { useProject } from './ProjectContext';
import type { Chat, ChatMessage } from '../types/chat';
import { loadChats, saveChats, createChat, updateChat, deleteChat } from '../services/chatStorage';

interface DateTimeRequest {
  toolCallId: string;
  message: string;
  type: 'range' | 'single';
  pendingMessages?: ChatMessage[]; // Store conversation state for resumption
}

interface ChatContextType {
  chats: Chat[];
  activeChatId: string | null;
  messages: ChatMessage[];
  userMessagesLength: number;
  setActiveChat: (chatId: string | null) => void;
  createNewChat: () => string;
  deleteChatById: (chatId: string) => void;
  addMessage: (msg: ChatMessage, overrideChatId?: string) => void;
  updateMessage: (id: string, updates: Partial<ChatMessage>, overrideChatId?: string) => void;
  updateActiveChat: (updates: Partial<Chat>, overrideChatId?: string) => void;
  clearHistory: () => void;
  isTyping: boolean;
  setIsTyping: (isTyping: boolean) => void;
  typingMessage: string | null;
  setTypingMessage: (message: string | null) => void;
  dateTimeRequest: DateTimeRequest | null;
  setDateTimeRequest: (request: DateTimeRequest | null) => void;
  submitDateTimeSelection: (toolCallId: string, startDate: string, endDate?: string) => void;
}

const ChatContext = createContext<ChatContextType | undefined>(undefined);

export function ChatProvider({ children }: { children: ReactNode }) {
  const { currentProject } = useProject();
  const projectId = currentProject?.id || null;

  const [chats, setChats] = useState<Chat[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [isTyping, setIsTyping] = useState(false);
  const [typingMessage, setTypingMessage] = useState<string | null>(null);

  // Date-time picker state
  const [dateTimeRequest, setDateTimeRequest] = useState<DateTimeRequest | null>(null);
  const [pendingDateTimeToolCallId, setPendingDateTimeToolCallId] = useState<string | null>(null);

  const submitDateTimeSelection = useCallback((toolCallId: string, startDate: string, endDate?: string) => {
    // This will be called by the DateTimePicker component
    // The actual tool response handling is in useChatCompletionv0_2
    setDateTimeRequest(null);
    setPendingDateTimeToolCallId(null);
  }, []);

  // Load chats when project changes — always start with a fresh new-chat state
  useEffect(() => {
    const loaded = loadChats(projectId);
    setChats(loaded);
    setActiveChatId(null);
  }, [projectId]);

  // Save chats whenever they change
  useEffect(() => {
    saveChats(projectId, chats);
  }, [chats, projectId]);

  const activeChat = chats.find((c) => c.id === activeChatId);
  const messages = activeChat?.messages || [];

  const setActiveChat = useCallback((chatId: string | null) => {
    setActiveChatId(chatId);
  }, []);

  const createNewChat = useCallback(() => {
    const newChat = createChat('New Chat');
    setChats((prev) => [newChat, ...prev]);
    setActiveChatId(newChat.id);
    return newChat.id;
  }, []);

  const deleteChatById = useCallback((chatId: string) => {
    setChats((prev) => {
      const filtered = prev.filter((c) => c.id !== chatId);
      // If we deleted the active chat, go to new-chat state
      if (activeChatId === chatId) {
        setActiveChatId(null);
      }
      return filtered;
    });
  }, [activeChatId]);

  const addMessage = useCallback((msg: ChatMessage, overrideChatId?: string) => {
    const chatId = overrideChatId || activeChatId;
    if (!chatId) return;
    setChats((prev) => {
      const chat = prev.find((c) => c.id === chatId);
      if (!chat) return prev;

      const updatedChat = {
        ...chat,
        messages: [...chat.messages, msg],
        updatedAt: Date.now(),
      };
      return prev.map((c) => (c.id === chatId ? updatedChat : c));
    });
  }, [activeChatId]);

  const updateMessage = useCallback((id: string, updates: Partial<ChatMessage>, overrideChatId?: string) => {
    const chatId = overrideChatId || activeChatId;
    if (!chatId) return;
    setChats((prev) => {
      const chat = prev.find((c) => c.id === chatId);
      if (!chat) return prev;

      const messageIndex = chat.messages.findIndex((msg) => msg.id === id);
      if (messageIndex === -1) return prev;

      // Keep messages up to and including the updated one, discard the tail
      const updatedMessages = chat.messages.slice(0, messageIndex + 1).map((msg, idx) =>
        idx === messageIndex ? { ...msg, ...updates } : msg
      );

      const updatedChat = {
        ...chat,
        messages: updatedMessages,
        updatedAt: Date.now(),
      };
      return prev.map((c) => (c.id === chatId ? updatedChat : c));
    });
  }, [activeChatId]);

  const updateActiveChat = useCallback((updates: Partial<Chat>, overrideChatId?: string) => {
    const chatId = overrideChatId || activeChatId;
    if (!chatId) return;
    setChats((prev) => {
      const chat = prev.find((c) => c.id === chatId);
      if (!chat) return prev;

      const updatedChat = {
        ...chat,
        ...updates,
        updatedAt: Date.now(),
      };
      return prev.map((c) => (c.id === chatId ? updatedChat : c));
    });
  }, [activeChatId]);

  const clearHistory = useCallback(() => {
    if (!activeChatId) return;
    setChats((prev) => {
      const chat = prev.find((c) => c.id === activeChatId);
      if (!chat) return prev;

      const updatedChat = {
        ...chat,
        messages: [],
        updatedAt: Date.now(),
      };
      return prev.map((c) => (c.id === activeChatId ? updatedChat : c));
    });
    setIsTyping(false);
    setTypingMessage(null);
  }, [activeChatId]);

  const value = useMemo(
    () => ({
      chats,
      activeChatId,
      messages,
      userMessagesLength: messages.filter((m) => m.role === 'user').length,
      setActiveChat,
      createNewChat,
      deleteChatById,
      addMessage,
      updateMessage,
      updateActiveChat,
      clearHistory,
      isTyping,
      setIsTyping,
      typingMessage,
      setTypingMessage,
      dateTimeRequest,
      setDateTimeRequest,
      submitDateTimeSelection,
    }),
    [chats, activeChatId, messages, isTyping, typingMessage, dateTimeRequest, submitDateTimeSelection]
  );

  return <ChatContext.Provider value={value}>{children}</ChatContext.Provider>;
}

function useChat() {
  const context = useContext(ChatContext);
  if (context === undefined) {
    throw new Error('useChat must be used within a ChatProvider');
  }
  return context;
}
