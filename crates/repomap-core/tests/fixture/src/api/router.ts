import { SessionStore } from "../auth/session";

const store = new SessionStore();

export function handleRefresh(req: { token: string }) {
  return { token: store.refresh(req.token) };
}
