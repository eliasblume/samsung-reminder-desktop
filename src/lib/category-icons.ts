import {
  Accessibility,
  AlarmClock,
  Baby,
  Bell,
  BookmarkCheck,
  BookOpen,
  Building2,
  CakeSlice,
  CarFront,
  ContactRound,
  CreditCard,
  Gamepad2,
  Gem,
  Gift,
  GraduationCap,
  Heart,
  House,
  Landmark,
  Leaf,
  Lightbulb,
  List,
  Luggage,
  Newspaper,
  Orbit,
  PaintbrushVertical,
  PawPrint,
  Pen,
  PillBottle,
  Plane,
  ReceiptText,
  School,
  ShieldKeyhole,
  ShoppingCart,
  Soup,
  Ticket,
  TrainFront,
  Trophy,
  UsersRound,
  Utensils,
  Volleyball,
  Wrench,
  Zap,
  type LucideIcon,
} from 'lucide-react';

export type SamsungCategoryIcon = {
  label: string;
  icon: LucideIcon;
};

export const categoryIcons: readonly SamsungCategoryIcon[] = [
  { label: 'Bell', icon: Bell },
  { label: 'Bullet point list', icon: List },
  { label: 'House', icon: House },
  { label: 'Heart', icon: Heart },
  { label: 'Diamond', icon: Gem },
  { label: 'Light bulb', icon: Lightbulb },
  { label: 'Wrapped present', icon: Gift },
  { label: 'Birthday cake', icon: CakeSlice },
  { label: 'Shopping cart', icon: ShoppingCart },
  { label: 'Fork and spoon', icon: Utensils },
  { label: 'Bowl with steam rising from it', icon: Soup },
  { label: 'Shield with keyhole', icon: ShieldKeyhole },
  { label: 'Bank-style building with columns', icon: Landmark },
  { label: 'Credit card', icon: CreditCard },
  { label: 'Receipt', icon: ReceiptText },
  { label: 'Ticket stub', icon: Ticket },
  { label: 'Airplane', icon: Plane },
  { label: 'Rolling suitcase', icon: Luggage },
  { label: 'Subway car', icon: TrainFront },
  { label: 'Car', icon: CarFront },
  { label: 'I.D. card', icon: ContactRound },
  { label: 'Game controller', icon: Gamepad2 },
  { label: 'Basketball', icon: Volleyball },
  { label: 'Trophy', icon: Trophy },
  { label: 'Person running', icon: Accessibility },
  { label: 'Lipstick with cap off', icon: PaintbrushVertical },
  { label: 'Office building', icon: Building2 },
  { label: 'School building', icon: School },
  { label: 'Graduation cap', icon: GraduationCap },
  { label: 'Open book', icon: BookOpen },
  { label: 'Pen', icon: Pen },
  { label: 'Alarm clock', icon: AlarmClock },
  { label: 'Pill bottle with pill', icon: PillBottle },
  { label: 'Group of three people', icon: UsersRound },
  { label: 'Smiling baby', icon: Baby },
  { label: 'Paw print', icon: PawPrint },
  { label: 'Leaf', icon: Leaf },
  { label: 'Ringed planet', icon: Orbit },
  { label: 'Bookmark with check', icon: BookmarkCheck },
  { label: 'Newspaper', icon: Newspaper },
  { label: 'Wrench', icon: Wrench },
  { label: 'Lightning bolt', icon: Zap },
];

export const categoryPalette = [
  '#9a87ff',
  '#ff7e7e',
  '#58d8db',
  '#ffd144',
  '#c4e383',
  '#e497df',
  '#ef9b4b',
  '#78beff',
] as const;

const categoryColorNames = [
  'Purple',
  'Red',
  'Turquoise',
  'Yellow',
  'Green',
  'Pink',
  'Orange',
  'Blue',
] as const;

export const categoryColorOrder = [1, 6, 3, 4, 7, 2, 5, 0] as const;

export function categoryIcon(index: number): SamsungCategoryIcon {
  return categoryIcons[index] ?? categoryIcons[0];
}

export function categoryColor(index: number): string {
  return categoryPalette[index] ?? categoryPalette[0];
}

export function categoryColorName(index: number): string {
  return categoryColorNames[index] ?? categoryColorNames[0];
}
