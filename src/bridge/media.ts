import type { MediaInfo } from './contracts';
/** Both arguments use the WebView's clock; Windows uptime never reaches here. */
export function mediaPosition(media: MediaInfo, receivedAtMs: number, nowMs: number) {
  const elapsed = media.playing ? Math.max(0, nowMs - receivedAtMs) / 1000 * media.rate : 0;
  return Math.min(Math.max(0, media.position + elapsed), media.duration);
}
