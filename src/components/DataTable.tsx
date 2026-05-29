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
  selected?: Set<string | number>;
  onSelect?: (keys: Set<string | number>) => void;
  onRowDoubleClick?: (row: T) => void;
  onContextMenu?: (e: React.MouseEvent, row: T) => void;
  emptyText?: string;
}

export default function DataTable<T>({ columns, data, rowKey, selected, onSelect, onRowDoubleClick, onContextMenu, emptyText = "暂无数据" }: Props<T>) {
  return (
    <div className="border border-gray-300 rounded overflow-hidden">
      <div className="overflow-auto max-h-[60vh]">
        <table className="w-full text-xs">
          <thead className="bg-gray-100 sticky top-0 z-10">
            <tr>
              {onSelect && (
                <th className="w-8 px-1 py-1.5 border-b border-gray-300">
                  <input type="checkbox" className="w-3.5 h-3.5" />
                </th>
              )}
              {columns.map(col => (
                <th key={col.key} className="text-left px-2 py-1.5 border-b border-gray-300 font-medium text-gray-600" style={{ width: col.width }}>
                  {col.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.length === 0 ? (
              <tr><td colSpan={columns.length + (onSelect ? 1 : 0)} className="text-center py-8 text-gray-400">{emptyText}</td></tr>
            ) : data.map(row => (
              <tr
                key={rowKey(row)}
                className="border-b border-gray-100 hover:bg-blue-50/50 cursor-default"
                onDoubleClick={() => onRowDoubleClick?.(row)}
                onContextMenu={e => onContextMenu?.(e, row)}
              >
                {onSelect && (
                  <td className="px-1 py-1">
                    <input type="checkbox" className="w-3.5 h-3.5" />
                  </td>
                )}
                {columns.map(col => (
                  <td key={col.key} className="px-2 py-1 text-gray-700">{col.render(row)}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
