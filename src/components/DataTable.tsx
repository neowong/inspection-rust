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
  emptyText?: string;
}

export default function DataTable<T>({ columns, data, rowKey, onRowDoubleClick, onContextMenu, emptyText = "暂无数据" }: Props<T>) {
  return (
    <div className="border border-[hsl(var(--border))] rounded-lg overflow-hidden">
      <div className="overflow-auto max-h-[60vh]">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-[hsl(var(--bg-hover))] sticky top-0 z-10">
              {columns.map(col => (
                <th
                  key={col.key}
                  className="text-left px-3 py-2 border-b border-[hsl(var(--border))] text-[11px] font-medium uppercase tracking-wide text-[hsl(var(--text-secondary))]"
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
                <td colSpan={columns.length} className="text-center py-12 text-[hsl(var(--text-tertiary))] text-sm">
                  {emptyText}
                </td>
              </tr>
            ) : (
              data.map(row => (
                <tr
                  key={rowKey(row)}
                  className="border-b border-[hsl(var(--border-light))] hover:bg-[hsl(var(--bg-hover))] transition-colors cursor-default"
                  onDoubleClick={() => onRowDoubleClick?.(row)}
                  onContextMenu={e => onContextMenu?.(e, row)}
                >
                  {columns.map(col => (
                    <td key={col.key} className="px-3 py-2 text-[hsl(var(--text-primary))]">
                      {col.render(row)}
                    </td>
                  ))}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
