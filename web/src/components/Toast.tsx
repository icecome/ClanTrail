import { createContext, useCallback, useContext, useState, type ReactNode } from 'react';

type ToastType = 'info' | 'error' | 'success';

interface ToastItem {
  id: number;
  msg: string;
  type: ToastType;
}

interface ToastCtx {
  show: (msg: string, type?: ToastType) => void;
}

const Ctx = createContext<ToastCtx>({ show: () => {} });

export function useToast() {
  return useContext(Ctx);
}

// 全局轻提示：替代 window.alert，用于错误/成功反馈
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const show = useCallback((msg: string, type: ToastType = 'info') => {
    const id = Date.now() + Math.random();
    setToasts((prev) => [...prev, { id, msg, type }]);
    window.setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 3000);
  }, []);

  return (
    <Ctx.Provider value={{ show }}>
      {children}
      <div className="toast-wrap">
        {toasts.map((t) => (
          <div key={t.id} className={`toast toast-${t.type}`}>
            {t.msg}
          </div>
        ))}
      </div>
    </Ctx.Provider>
  );
}
