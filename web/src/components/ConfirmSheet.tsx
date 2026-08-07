import BottomSheet from './BottomSheet';

interface Props {
  open: boolean;
  title?: string;
  message?: string;
  confirmText?: string;
  cancelText?: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

// 底部确认弹层：用于删除等二次确认，替代 window.confirm
export default function ConfirmSheet({
  open,
  title,
  message,
  confirmText = '确定',
  cancelText = '取消',
  danger,
  onConfirm,
  onCancel,
}: Props) {
  return (
    <BottomSheet open={open} title={title} onClose={onCancel}>
      {message && <p className="sheet-message">{message}</p>}
      <div className="sheet-actions">
        <button className="sheet-btn" onClick={onCancel}>
          {cancelText}
        </button>
        <button
          className={`sheet-btn ${danger ? 'danger' : 'primary'}`}
          onClick={onConfirm}
        >
          {confirmText}
        </button>
      </div>
    </BottomSheet>
  );
}
