type BatchStatus = "pending" | "running" | "completed" | "failed" | "stopped" | "partially_completed";

export function batchStatusColor(status: string): BatchStatus {
  if (status === "pending" || status === "waiting") return "pending";
  if (status === "running" || status === "in_progress") return "running";
  if (status === "completed") return "completed";
  if (status === "failed") return "failed";
  if (status === "stopped" || status === "paused") return "stopped";
  if (status === "partially_completed") return "partially_completed";
  return "pending";
}
