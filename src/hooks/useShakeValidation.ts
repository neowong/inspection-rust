import { useState, useCallback, useRef, useEffect } from "react";

/**
 * 表单验证抖动效果 hook
 * 用于在验证失败时触发字段的抖动动画
 */
export function useShakeValidation() {
  const [shakeFields, setShakeFields] = useState<Set<string>>(new Set());
  const timersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  // 组件卸载时清理所有未触发的定时器，防止向已卸载组件 setState
  useEffect(() => () => {
    timersRef.current.forEach((t) => clearTimeout(t));
    timersRef.current.clear();
  }, []);

  const triggerShake = useCallback((field: string) => {
    // 清除该字段之前的定时器，避免竞态
    const existing = timersRef.current.get(field);
    if (existing) clearTimeout(existing);

    setShakeFields((prev) => new Set(prev).add(field));
    const timer = setTimeout(() => {
      setShakeFields((prev) => {
        const next = new Set(prev);
        next.delete(field);
        return next;
      });
      timersRef.current.delete(field);
    }, 600);
    timersRef.current.set(field, timer);
  }, []);

  return { shakeFields, triggerShake };
}
