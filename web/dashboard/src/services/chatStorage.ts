import type { Chat } from '../types/chat';
import { generateUUID } from '../utils/uuid';

const STORAGE_KEY_PREFIX = 'hyper_analytics_chats_';

function getStorageKey(projectId: string | null): string {
  return `${STORAGE_KEY_PREFIX}${projectId || 'default'}`;
}

export function saveChats(projectId: string | null, chats: Chat[]): void {
  if (!projectId) return;
  try {
    localStorage.setItem(getStorageKey(projectId), JSON.stringify(chats));
  } catch (error) {
    console.error('Failed to save chats to localStorage:', error);
  }
}

export function loadChats(projectId: string | null): Chat[] {
  if (!projectId) return [];
  try {
    const data = localStorage.getItem(getStorageKey(projectId));
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

export function updateChat(projectId: string | null, id: string, updates: Partial<Chat>): Chat | null {
  if (!projectId) return null;
  const chats = loadChats(projectId);
  const index = chats.findIndex((chat) => chat.id === id);

  if (index === -1) {
    return null;
  }

  chats[index] = {
    ...chats[index],
    ...updates,
    updatedAt: Date.now(),
  };

  saveChats(projectId, chats);
  return chats[index];
}

export function deleteChat(projectId: string | null, id: string): boolean {
  if (!projectId) return false;
  const chats = loadChats(projectId);
  const filteredChats = chats.filter((chat) => chat.id !== id);

  if (filteredChats.length === chats.length) {
    return false;
  }

  saveChats(projectId, filteredChats);
  return true;
}
