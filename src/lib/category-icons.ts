import { t } from '../i18n';
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
  { get label() { return t("icon.0"); }, icon: Bell },
  { get label() { return t("icon.1"); }, icon: List },
  { get label() { return t("icon.2"); }, icon: House },
  { get label() { return t("icon.3"); }, icon: Heart },
  { get label() { return t("icon.4"); }, icon: Gem },
  { get label() { return t("icon.5"); }, icon: Lightbulb },
  { get label() { return t("icon.6"); }, icon: Gift },
  { get label() { return t("icon.7"); }, icon: CakeSlice },
  { get label() { return t("icon.8"); }, icon: ShoppingCart },
  { get label() { return t("icon.9"); }, icon: Utensils },
  { get label() { return t("icon.10"); }, icon: Soup },
  { get label() { return t("icon.11"); }, icon: ShieldKeyhole },
  { get label() { return t("icon.12"); }, icon: Landmark },
  { get label() { return t("icon.13"); }, icon: CreditCard },
  { get label() { return t("icon.14"); }, icon: ReceiptText },
  { get label() { return t("icon.15"); }, icon: Ticket },
  { get label() { return t("icon.16"); }, icon: Plane },
  { get label() { return t("icon.17"); }, icon: Luggage },
  { get label() { return t("icon.18"); }, icon: TrainFront },
  { get label() { return t("icon.19"); }, icon: CarFront },
  { get label() { return t("icon.20"); }, icon: ContactRound },
  { get label() { return t("icon.21"); }, icon: Gamepad2 },
  { get label() { return t("icon.22"); }, icon: Volleyball },
  { get label() { return t("icon.23"); }, icon: Trophy },
  { get label() { return t("icon.24"); }, icon: Accessibility },
  { get label() { return t("icon.25"); }, icon: PaintbrushVertical },
  { get label() { return t("icon.26"); }, icon: Building2 },
  { get label() { return t("icon.27"); }, icon: School },
  { get label() { return t("icon.28"); }, icon: GraduationCap },
  { get label() { return t("icon.29"); }, icon: BookOpen },
  { get label() { return t("icon.30"); }, icon: Pen },
  { get label() { return t("icon.31"); }, icon: AlarmClock },
  { get label() { return t("icon.32"); }, icon: PillBottle },
  { get label() { return t("icon.33"); }, icon: UsersRound },
  { get label() { return t("icon.34"); }, icon: Baby },
  { get label() { return t("icon.35"); }, icon: PawPrint },
  { get label() { return t("icon.36"); }, icon: Leaf },
  { get label() { return t("icon.37"); }, icon: Orbit },
  { get label() { return t("icon.38"); }, icon: BookmarkCheck },
  { get label() { return t("icon.39"); }, icon: Newspaper },
  { get label() { return t("icon.40"); }, icon: Wrench },
  { get label() { return t("icon.41"); }, icon: Zap },
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
  return t(`color.${index >= 0 && index < categoryColorNames.length ? index : 0}`);
}
