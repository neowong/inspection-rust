interface Props {
  children: React.ReactNode;
}

export default function Toolbar({ children }: Props) {
  return (
    <div className="flex items-center gap-2 flex-wrap mb-4">
      {children}
    </div>
  );
}
