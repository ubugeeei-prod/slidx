/**
 * The audience channel: questions and reactions from the room, opt in.
 *
 * Two halves that share one protocol. The Vite plugin always names this
 * package so a reachable check can see the client; nothing is emitted onto a
 * page unless the deck opted in. A default deck still ships nothing extra and
 * makes no requests — the feature is a Worker somebody chose to deploy and a
 * client somebody chose to load, not a default that quietly phones home.
 *
 * The whole entry point is here so a deployment can pull the Worker and a deck
 * can pull the client from the same package, and neither can drift away from
 * the protocol they both depend on.
 */

export { createBackoff } from "./backoff";
export type { Backoff, BackoffOptions } from "./backoff";

export { createAudienceChannel, socketUrl } from "./client";
export type {
  AudienceChannel,
  AudienceChannelOptions,
  ChannelSocket,
  ChannelSocketHandlers,
  ChannelStatus,
  Scheduler,
  SocketFactory,
} from "./client";

export { createParticipant, createTokenBucket } from "./participant";
export type {
  Participant,
  ParticipantOptions,
  TokenBucket,
  TokenBucketOptions,
} from "./participant";

export {
  checkIdentifier,
  checkName,
  checkText,
  emptyTally,
  isReactionKind,
  isRoomSlug,
  LIMITS,
  parseServerFrame,
  PROTOCOL_VERSION,
  REACTION_KINDS,
  sanitizeText,
  textLength,
  validateClientMessage,
  validateFrame,
} from "./protocol";
export type {
  ClientMessage,
  ClientMessageType,
  ModerationMode,
  PublishedQuestion,
  ReactionKind,
  ReactionTally,
  RejectionReason,
  RoomEndReason,
  RoomSnapshot,
  ServerMessage,
  Validation,
} from "./protocol";

export { pendingQuestions, publishedQuestions } from "./questions";
export type { StoredQuestion } from "./questions";

export { createRoom, ROOM_LIFETIME } from "./room";
export type {
  HostOutcome,
  OpenOptions,
  OpenOutcome,
  Room,
  RoomOptions,
  RoomStorage,
  SubmitOutcome,
} from "./room";

export { createRelayHub, isSessionId, readRelayFrame, SESSION_HEX_LENGTH } from "./relay";
export type { HubOutcome, JoinFrame, RelayFrame, RelayHub } from "./relay";

export { routeSessionRequest, splitSessionPath } from "./relay-routes";
export type { SessionRouteContext } from "./relay-routes";

export { routeRoomRequest, splitRoomPath } from "./routes";
export type { RouteContext } from "./routes";

export { receiveFrame } from "./session";
export type { Session } from "./session";

export {
  audienceWorker,
  AudienceRoom,
  RemoteSession,
  createRoomHub,
  handleFetch,
  receiveRelay,
} from "./worker";
export type {
  AudienceEnv,
  DurableObjectNamespaceLike,
  DurableObjectStateLike,
  RoomHub,
  Sink,
} from "./worker";
