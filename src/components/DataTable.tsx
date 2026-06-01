import React from "react";

interface Column<T> {
  key: string;
  header: string;
  width?: string;
  render: (row: T) => React.ReactNode;
}

interface Props<T> {
  columns: Column<T>[];
  data: T[];
  rowKey: (row: T) => string | number;
  onRowDoubleClick?: (row: T) => void;
  onContextMenu?: (e: React.MouseEvent, row: T) => void;
  onRowClick?: (row: T) => void;
  selectedKey?: string | number | null;
  emptyText?: string;
  className?: string;
}

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
  return (
    <div className="border border-[hsl(var(--border))] rounded-lg overflow-hidden">
      <div className="overflow-auto max-h-[60vh]">
        <table className={`w-full text-sm ${className ?? ""}`}>
          <thead>
            <tr className="bg-[hsl(var(--bg-hover))] sticky top-0 z-10">
              {columns.map((col) => (
                <th
                  key={col.key}
                  className="text-left px-3 py-2 border-b border-[hsl(var(--border))] text-xs font-medium uppercase tracking-wide text-[hsl(var(--text-secondary))]"
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
                    {columns.map((col) => (
                      <td
                        key={col.key}
                        className="px-3 py-2 border-b border-[hsl(var(--border-light))] text-sm text-[hsl(var(--text-primary))]"
                      >
                        {col.render(row)}
                      </td>
                    ))}
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
