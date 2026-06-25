import { useCallback, type KeyboardEvent } from "react";

/**
 * 表单 Enter 导航 hook
 *
 * 用法：在所有需要参与导航的 Input 上加 onKeyDown={onInputKeyDown}。
 * Enter 从当前 input 跳到下一个 input，最后一个触发 onSave。
 * Select / button 等不参与导航。
 *
 *   const { onInputKeyDown, containerRef } = useEnterNavigation(handleSave);
 *   <div ref={containerRef}>
 *     <Input onKeyDown={onInputKeyDown} ... />
 *     <Input onKeyDown={onInputKeyDown} ... />  ← 最后一个，Enter 触发保存
 *   </div>
 */
export function useEnterNavigation(onSave: () => void) {
  // 用普通 ref 变量（useRef 的 .current 是 readonly，回调赋值用外层变量）
  let containerEl: HTMLDivElement | null = null;
  const containerRef = useCallback((el: HTMLDivElement | null) => {
    containerEl = el;
  }, []);

  const onInputKeyDown = useCallback(
    (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key !== "Enter") return;
      e.preventDefault();

      const container = containerEl;
      if (!container) { onSave(); return; }

      const inputs = Array.from(
        container.querySelectorAll<HTMLInputElement>('input')
      ).filter((el) => {
        const t = el.type.toLowerCase();
        // 只要 text/password/无 type（默认 text），跳过 checkbox/radio/hidden/submit 等
        return t === "text" || t === "password" || t === "";
      });

      const idx = inputs.indexOf(e.currentTarget);
      if (idx === -1) return;

      // 找下一个可见 input
      const next = inputs.slice(idx + 1).find((el) => el.offsetParent !== null);

      if (next) {
        next.focus();
        // 选中已有内容方便直接覆盖输入
        try { next.select(); } catch { /* ignore */ }
      } else {
        onSave();
      }
    },
    [onSave]
  );

  return { onInputKeyDown, containerRef };
}
