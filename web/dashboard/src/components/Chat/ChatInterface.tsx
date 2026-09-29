import { Box, Typography } from '@mui/material';
import { SmartDropdown } from './SmartDropdown';
import type { ChatType } from '../../types/chat';
import { ChatTypes, ChatTypeOptions } from '../../types/chat';
import type { ReactNode } from 'react';
import {CornerDownRight, Send} from 'lucide-react'
import { useRef, useEffect, useState, useCallback } from 'react';
import {
  colorInk,
  colorInk20,
  fontFamilyDisplay,
  fontFamilyBody,
} from '../../theme/tokens';

interface ChatInterfaceProps {
  groups: ReactNode[][];
  onSendMessage: (content: string, chatType: ChatType) => void;
  isTyping?: boolean;
  placeholder?: string;
  emptySubtitle?: string;
  chatType?: ChatType;
  onChatTypeChange?: (chatType: ChatType) => boolean;
}

export function ChatInterface({
  groups,
  onSendMessage,
  isTyping = false,
  placeholder = 'Ask a question…',
  emptySubtitle = '',
  chatType,
  onChatTypeChange,
}: ChatInterfaceProps) {
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const endRef = useRef<HTMLDivElement>(null);
  const lastUserMessageRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [inputContent, setInputContent] = useState('');
  const [scrollContainerHeight, setScrollContainerHeight] = useState(0);
  const effectiveType = chatType ?? ChatTypes.GENERAL;

  const isScrollViewReady = scrollContainerHeight > 0;

  useEffect(() => {
    const updateHeight = () => {
      setScrollContainerHeight(scrollContainerRef.current?.clientHeight ?? 0);
    };
    updateHeight();
    window.addEventListener('resize', updateHeight);
    return () => window.removeEventListener('resize', updateHeight);
  }, []);

  useEffect(() => {
    if (scrollContainerRef.current) {
      setScrollContainerHeight(scrollContainerRef.current.clientHeight);
    }
  }, [groups.length]);

  const handleSend = useCallback(() => {
    if (!inputContent.trim() || isTyping) return;

    onSendMessage(inputContent.trim(), effectiveType);
    setInputContent('');
    // Reset textarea height
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
  }, [inputContent, isTyping, onSendMessage, effectiveType]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend]
  );

  const scrollToLastUserMessage = useCallback(() => {
    // quickfix to a bug that writes ui after scroll starts so scroll is not accurate
    // so we are waiting for sometime before triggering
    setTimeout(()=>{
      if (scrollContainerRef.current && lastUserMessageRef.current) {
        const targetTop = Math.max(lastUserMessageRef.current.offsetTop - 40, 0);
        scrollContainerRef.current.scrollTo({ top: targetTop, behavior: 'smooth' });
      }
    }, 10);

    if (scrollContainerRef.current && lastUserMessageRef.current) {
      return;
    }

    endRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, []);

  useEffect(() => {
    const frame = window.requestAnimationFrame(() => {
      scrollToLastUserMessage();
    });

    return () => window.cancelAnimationFrame(frame);
  }, [groups.length, isScrollViewReady, scrollToLastUserMessage]);

  const handleModeChange = (value: string) => {
    const newType = value as ChatType;
    onChatTypeChange?.(newType);
  };

  const inputBox = (
    <Box sx={{ padding: '0px 32px 24px', background: `linear-gradient(transparent, #F8F6F0 20%)` }}>
      <Box sx={{ maxWidth: 900, mx: 'auto' }}>
        <Box
          sx={{
            display: 'flex',
            flexDirection: 'column',
            bgcolor: '#fff',
            border: '1px solid rgba(24,22,15,0.15)',
            borderRadius: '5px',
            padding: '12px 14px',
            boxShadow: '0 2px 12px rgba(24,22,15,0.08)',
          }}
        >
          <Box
            component="textarea"
            ref={textareaRef}
            rows={1}
            placeholder={placeholder}
            value={inputContent}
            autoFocus
            onChange={(e) => {
              setInputContent(e.target.value);
              e.target.style.height = 'auto';
              e.target.style.height = Math.min(e.target.scrollHeight, 160) + 'px';
            }}
            onKeyDown={handleKeyDown}
            sx={{
              width: '100%',
              border: 'none',
              outline: 'none',
              resize: 'none',
              fontFamily: fontFamilyBody,
              fontSize: '17px',
              color: colorInk,
              bgcolor: 'transparent',
              lineHeight: 1.5,
              minHeight: '28px',
              maxHeight: '160px',
              marginBottom: '12px',
              '&::placeholder': { color: '#928F88' },
            }}
          />
          <Box
            sx={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
            }}
          >
            <SmartDropdown
              value={effectiveType}
              options={[
                {value: 'general', label: 'General', subtext: 'Ask questions about your data and get metrics.'},
                {value: 'create_dashboard', label: 'Create Dashboard', subtext: 'Conversation aims to create a dashboard'}
              ]}
              onChange={handleModeChange}
              buttonStyles={effectiveType === 'create_dashboard' ? {
                color: '#4287f5',
              } : undefined}
            />
            <Box
              component="button"
              onClick={handleSend}
              disabled={!inputContent.trim() || isTyping}
              sx={{
                width: 34,
                height: 34,
                bgcolor: inputContent.trim() && !isTyping ? '#4287f5' : '#E8E4DA',
                color: inputContent.trim() ? '#fff' : '#928F88',
                border: 'none',
                borderRadius: '3px',
                flexShrink: 0,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                cursor: inputContent.trim() ? 'pointer' : 'not-allowed',
                transition: 'all 0.15s',
                padding: 0,
                '&:hover': {
                  bgcolor: inputContent.trim() ? '#2563eb' : '#E8E4DA',
                },
                '&:disabled': {
                  bgcolor: '#E8E4DA',
                  color: '#928F88',
                  cursor: 'not-allowed',
                },
              }}
            >
            <CornerDownRight size={20}/>
            </Box>
          </Box>
        </Box>
      </Box>
    </Box>
  );

  if (groups.length === 0) {
    return (
      <Box
        sx={{
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          p: { xs: 2, sm: 4, md: 6 },
          backgroundImage: `radial-gradient(circle, ${colorInk20} 0.5px, transparent 0.5px)`,
          backgroundSize: '24px 24px',
        }}
      >
        <Box sx={{ textAlign: 'center', mb: 4 }}>
          <Typography
            component="h2"
            sx={{
              fontFamily: fontFamilyDisplay,
              fontSize: '32px',
              fontWeight: 400,
              color: colorInk,
              letterSpacing: '-0.8px',
              mb: 1.25,
            }}
          >
            {effectiveType === 'create_dashboard' ? 'What dashboard would you like to create?' : 'What would you like to explore?'}
          </Typography>
          {emptySubtitle && (
            <Typography
              sx={{
                fontSize: '15px',
                color: '#928F88',
                fontWeight: 300,
                lineHeight: 1.7,
              }}
            >
              {emptySubtitle}
            </Typography>
          )}
        </Box>
        <Box sx={{ width: '100%', maxWidth: 900 }}>{inputBox}</Box>
      </Box>
    );
  }

  return (
    <Box sx={{ flex: 1, display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <Box
        ref={scrollContainerRef}
        sx={{
          flex: 1,
          overflowY: 'auto',
          overflowX: 'hidden',
          px: { xs: 2, sm: 4, md: 6 },
          backgroundImage: `radial-gradient(circle, ${colorInk20} 0.5px, transparent 0.5px)`,
          backgroundSize: '24px 24px',
        }}
      >
        <Box sx={{ maxWidth: 800, mx: 'auto' }}>
          <Box sx={{ mt: { xs: 2, sm: 4, md: 6 } }} />
          {groups.map((group, gi) => {
            const isLast = gi === groups.length - 1;
            return (
              <Box
                key={`group-${gi}`}
                ref={isLast ? lastUserMessageRef : null}
                sx={{
                  minHeight: isLast && isScrollViewReady ? scrollContainerHeight - 50 : 'auto',
                  paddingBottom: { xs: 2, sm: 4, md: 6 },
                }}
              >
                {group.map((node, ni) => (
                  <Box key={`${gi}-${ni}`}>{node}</Box>
                ))}
              </Box>
            );
          })}
          <div ref={endRef} />
        </Box>
      </Box>
      <Box sx={{ flexShrink: 0 }}>{inputBox}</Box>
    </Box>
  );
}
