import type { LucideIcon } from 'lucide-react';
import { Backpack, BookOpen, Coffee, Headphones, Shirt, Watch } from 'lucide-react';

export type Product = {
  id: string;
  name: string;
  category: string;
  /** In rupees. */
  price: number;
  icon: LucideIcon;
  /** Background of the product's tile. */
  tint: string;
};

export const CURRENCY = 'INR';

export const PRODUCTS: Product[] = [
  { id: 'coffee-beans', name: 'Single-origin coffee beans', category: 'Pantry', price: 649, icon: Coffee, tint: '#F3E6D8' },
  { id: 'cotton-tee', name: 'Organic cotton tee', category: 'Apparel', price: 899, icon: Shirt, tint: '#E3ECF7' },
  { id: 'wireless-headphones', name: 'Wireless headphones', category: 'Electronics', price: 4999, icon: Headphones, tint: '#E6E4F2' },
  { id: 'canvas-backpack', name: 'Canvas backpack', category: 'Bags', price: 2499, icon: Backpack, tint: '#E4EFE6' },
  { id: 'analog-watch', name: 'Minimal analog watch', category: 'Accessories', price: 3299, icon: Watch, tint: '#F5E2DC' },
  { id: 'dot-grid-notebook', name: 'Dot-grid notebook', category: 'Stationery', price: 349, icon: BookOpen, tint: '#F2EEDB' },
];

const priceFormat = new Intl.NumberFormat('en-IN', {
  style: 'currency',
  currency: CURRENCY,
  maximumFractionDigits: 0,
});

export const formatPrice = (amount: number): string => priceFormat.format(amount);
