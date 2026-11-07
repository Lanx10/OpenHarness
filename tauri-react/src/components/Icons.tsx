import {
  HomeIcon,
  Cog8ToothIcon,
  PlusIcon,
  FolderIcon,
  XMarkIcon,
  MinusIcon,
  SquaresPlusIcon,
  ComputerDesktopIcon,
  CheckIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  ChevronLeftIcon,
  PaperClipIcon,
  PaperAirplaneIcon,
  ArrowUpIcon,
} from '@heroicons/react/20/solid';

export type IconName =
  | 'home' | 'gear' | 'plus' | 'folder' | 'close'
  | 'minus' | 'square' | 'down' | 'right' | 'left' | 'local' | 'check'
  | 'attach' | 'send' | 'select' | 'selected';

const ICONS = {
  home: HomeIcon,
  gear: Cog8ToothIcon,
  plus: PlusIcon,
  folder: FolderIcon,
  close: XMarkIcon,
  minus: MinusIcon,
  square: SquaresPlusIcon,
  down: ChevronDownIcon,
  right: ChevronRightIcon,
  left: ChevronLeftIcon,
  local: ComputerDesktopIcon,
  check: CheckIcon,
  attach: PaperClipIcon,
  send: PaperAirplaneIcon,
  select: ArrowUpIcon,
  selected: CheckIcon,
} satisfies Record<IconName, React.ComponentType<React.SVGProps<SVGSVGElement>>>;

export function I({name, size = 16, className}: {name: IconName; size?: number; className?: string}) {
  const Component = ICONS[name];
  return <Component aria-hidden="true" className={className} width={size} height={size}/>;
}
