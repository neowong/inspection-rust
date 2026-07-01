import { useEffect, useRef } from "react";
import type { ReactNode, MouseEvent } from "react";

interface Column<T> {
  key: string;
  header: string | ReactNode;
  width?: string;
  /** 默认所有单元格强制单行（whitespace-nowrap）。设为 true 则允许折行（用于备注/输出等长文本列）。 */
  wrap?: boolean;
  /** 单元格最大宽度（CSS 值），默认 24rem。超出时显示渐变省略 + hover tooltip。设为 "none" 取消限制。 */
  maxWidth?: string;
  /** 设为 true 的列不做截断（比如操作列、徽章列），保持原始布局 */
  noTruncate?: boolean;
  render: (row: T) => ReactNode;
}

interface Props<T> {
  columns: Column<T>[];
  data: T[];
  rowKey: (row: T) => string | number;
  onRowDoubleClick?: (row: T) => void;
  onContextMenu?: (e: MouseEvent, row: T) => void;
  onRowClick?: (row: T) => void;
  selectedKey?: string | number | null;
  emptyText?: string;
  className?: string;
}

/**
 * 当鼠标进入截断的单元格内容时，刷新 title（避免初始检测后内容动态变化）。
 */
const handleCellMouseEnter = (e: MouseEvent<HTMLDivElement>) => {
  const t = e.currentTarget;
  if (t.scrollWidth > t.clientWidth) {
    t.dataset.overflow = "true";
    t.title = t.textContent ?? "";
  } else {
    delete t.dataset.overflow;
    t.title = "";
  }
};

export default function DataTable<T>({
  columns,
  data,
  rowKey,
  onRowDoubleClick,
  onContextMenu,
  onRowClick,
  selectedKey,
  className,
  emptyText = "暂无数据",
}: Props<T>) {
  const containerRef = useRef<HTMLDivElement>(null);

  // 数据或列定义变化后扫描所有截断单元格，标记溢出的元素以启用渐变效果。
  // 同时监听容器尺寸变化（窗口大小调整、表格列宽变化），重新检测。
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const detect = () => {
      const cells = container.querySelectorAll<HTMLDivElement>(".table-cell-truncate");
      cells.forEach((el) => {
        if (el.scrollWidth > el.clientWidth) {
          el.dataset.overflow = "true";
          if (!el.title) el.title = el.textContent ?? "";
        } else {
          delete el.dataset.overflow;
          el.title = "";
        }
      });
    };
    // 等下一帧让浏览器完成布局
    const raf = requestAnimationFrame(detect);
    const ro = new ResizeObserver(detect);
    ro.observe(container);
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
    };
  }, [data, columns]);

  return (
    <div ref={containerRef} className="border border-[hsl(var(--border))] rounded-lg overflow-hidden">
      <div className="overflow-auto max-h-[60vh]">
        <table className={`w-full text-sm ${className ?? ""}`}>
          <thead>
            <tr className="bg-[hsl(var(--bg-hover))] sticky top-0 z-10">
              {columns.map((col) => (
                <th
                  key={col.key}
                  className="text-left px-3 py-2 border-b border-[hsl(var(--border))] text-xs font-medium uppercase tracking-wide text-[hsl(var(--text-secondary))] whitespace-nowrap"
                  style={{ width: col.width }}
                >
                  {col.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.length === 0 ? (
              <tr>
                <td
                  colSpan={columns.length}
                  className="text-center py-12 text-sm text-[hsl(var(--text-tertiary))]"
                >
                  {emptyText}
                </td>
              </tr>
            ) : (
              data.map((row) => {
                const key = rowKey(row);
                const selected =
                  selectedKey !== undefined &&
                  selectedKey !== null &&
                  selectedKey === key;
                return (
                  <tr
                    key={key}
                    onClick={() => onRowClick?.(row)}
                    onDoubleClick={() => onRowDoubleClick?.(row)}
                    onContextMenu={(e) => onContextMenu?.(e, row)}
                    className={`${onRowClick ? "cursor-pointer" : ""} transition-colors ${
                      selected
                        ? "bg-[hsl(var(--accent-subtle))]"
                        : "hover:bg-[hsl(var(--bg-hover))]"
                    }`}
                  >
                    {columns.map((col) => {
                      const truncate = !col.wrap && !col.noTruncate;
                      const maxWidth =
                        truncate && col.maxWidth !== "none"
                          ? col.maxWidth ?? "24rem"
                          : undefined;
                      return (
                        <td
                          key={col.key}
                          className={`px-3 py-2 border-b border-[hsl(var(--border-light))] text-sm text-[hsl(var(--text-primary))] align-middle ${
                            col.wrap ? "" : "whitespace-nowrap"
                          }`}
                          style={maxWidth ? { maxWidth } : undefined}
                        >
                          {truncate ? (
                            <div
                              className="table-cell-truncate"
                              onMouseEnter={handleCellMouseEnter}
                            >
                              {col.render(row)}
                            </div>
                          ) : (
                            col.render(row)
                          )}
                        </td>
                      );
                    })}
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
