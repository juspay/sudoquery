import type { Chat } from '../types/chat';
import { generateUUID } from '../utils/uuid';

const STORAGE_KEY_PREFIX = 'hyper_analytics_chats_';

function getStorageKey(workspaceId: string | null): string {
  return `${STORAGE_KEY_PREFIX}${workspaceId || 'default'}`;
}

export function saveChats(workspaceId: string | null, chats: Chat[]): void {
  if (!workspaceId) return;
  try {
    localStorage.setItem(getStorageKey(workspaceId), JSON.stringify(chats));
  } catch (error) {
    console.error('Failed to save chats to localStorage:', error);
  }
}

export function loadChats(workspaceId: string | null): Chat[] {
  if (!workspaceId) return [];
  try {
    const data = localStorage.getItem(getStorageKey(workspaceId));
    if (!data) {
      return [];
    }
    return JSON.parse(data) as Chat[];
  } catch (error) {
    console.error('Failed to load chats from localStorage:', error);
    return [];
  }
}

export function createChat(title: string = 'New Chat'): Chat {
  return {
    id: generateUUID(),
    title,
    messages: [],
    createdAt: Date.now(),
    updatedAt: Date.now(),
    version: 0.2
  };
}

export function updateChat(workspaceId: string | null, id: string, updates: Partial<Chat>): Chat | null {
  if (!workspaceId) return null;
  const chats = loadChats(workspaceId);
  const index = chats.findIndex((chat) => chat.id === id);

  if (index === -1) {
    return null;
  }

  chats[index] = {
    ...chats[index],
    ...updates,
    updatedAt: Date.now(),
  };

  saveChats(workspaceId, chats);
  return chats[index];
}

export function deleteChat(workspaceId: string | null, id: string): boolean {
  if (!workspaceId) return false;
  const chats = loadChats(workspaceId);
  const filteredChats = chats.filter((chat) => chat.id !== id);

  if (filteredChats.length === chats.length) {
    return false;
  }

  saveChats(workspaceId, filteredChats);
  return true;
}
