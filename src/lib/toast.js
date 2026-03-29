let toastId = 0;
let listeners = [];

export let toasts = [];

function notify() {
  listeners.forEach(fn => fn(toasts));
}

export function addToast(message, type = 'info', duration = type === 'error' ? 10000 : 3000) {
  const id = ++toastId;
  toasts = [...toasts, { id, message, type }];
  notify();
  setTimeout(() => {
    toasts = toasts.filter(t => t.id !== id);
    notify();
  }, duration);
}

export function onToastChange(fn) {
  listeners.push(fn);
  return () => { listeners = listeners.filter(l => l !== fn); };
}
