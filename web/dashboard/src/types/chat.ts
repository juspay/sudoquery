import { z } from 'zod';

/**
 * ChatType enum matching the Rust backend's chat_type database type
 * Used to categorize conversations between general queries and dashboard creation
 */
const ChatTypeSchema = z.enum(['general', 'create_dashboard']);

/**
 * ChatType type derived from Zod schema
 */
export type ChatType = z.infer<typeof ChatTypeSchema>;

/**
 * Chat interface representing a chat conversation
 */
export interface Chat {
  id: string;
  title: string;
  createdAt: number;
  chat_type: ChatType;
}

/**
 * Helper constants for type safety
 */
export const ChatTypes = {
  GENERAL: 'general' as const,
  CREATE_DASHBOARD: 'create_dashboard' as const,
};

/**
 * Human-readable labels for ChatType values
 */
export const ChatTypeLabels: Record<ChatType, string> = {
  general: 'General',
  create_dashboard: 'Create Dashboard',
};

/**
 * Descriptions for each chat type
 */
const ChatTypeDescriptions: Record<ChatType, string> = {
  general: 'Ask questions about your data and get metrics.',
  create_dashboard: 'Conversation aims to create a dashboard',
};

/**
 * Full option objects for use in dropdowns
 */
export const ChatTypeOptions = Object.values(ChatTypes).map((value) => ({
  value,
  label: ChatTypeLabels[value],
  subtext: ChatTypeDescriptions[value],
}));
