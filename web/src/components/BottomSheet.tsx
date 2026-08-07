import type { ReactNode } from 'react';

interface Props {
  open: boolean;
  title?: string;
  onClose: () => void;
  children: ReactNode;
}

// 移动端底部弹出面板：用于选择、确认等轻交互，替代浏览器原生弹窗
export default function BottomSheet({ open, title, onClose, children }: Props) {
  if (!open) return null;
  return (
    <div className="sheet-backdrop" onClick={onClose}>
      <div className="sheet" onClick={(e) => e.stopPropagation()}>
        {title && <div className="sheet-title">{title}</div>}
        <div className="sheet-body">{children}</div>
      </div>
    </div>
  );
}
