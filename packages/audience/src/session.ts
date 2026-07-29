/**
 * One connected participant, from the room's side.
 *
 * The step between "some bytes arrived on a socket" and "the room changed". It
 * exists as its own module so that step can be tested without a socket at all:
 * everything a WebSocket contributes is a string going in and a string coming
 * out, and neither of those needs a network to exercise.
 *
 * The rule it enforces is that a sender always learns what happened to what
 * they sent. A question that silently vanishes gets asked again, and again,
 * which is how a moderation queue fills up with the same question four times.
 */

import { validateFrame, type ServerMessage } from "./protocol";
import type { Participant } from "./participant";
import type { Room } from "./room";

export interface Session {
  room: Room;
  participant: Participant;
  /** To this connection only. */
  reply(message: ServerMessage): void;
  /** Re-sends state to everyone, each in the view they are entitled to. */
  broadcast(): Promise<void>;
}

/** Handles one inbound frame. Never throws: a bad frame is an answer, not a crash. */
export async function receiveFrame(raw: string, session: Session): Promise<void> {
  const message = validateFrame(raw);
  if (!message.ok) {
    session.reply({ type: "rejected", reason: message.reason });
    return;
  }

  const outcome = await session.room.submit(message.value, session.participant);
  if (!outcome.ok) {
    session.reply({ type: "rejected", reason: outcome.reason });
    return;
  }

  // Only a question gets an acknowledgement of its own, and only because its
  // author needs to know whether it is waiting for the speaker or already up.
  // For an upvote or a reaction the snapshot that follows *is* the answer.
  if (outcome.effect !== "counted") {
    session.reply({ type: "accepted", held: outcome.effect === "held" });
  }

  await session.broadcast();
}
