// client/src/types/ragflowTypes.ts
export interface RagflowChatRequestPayload {
  question: string;
  sessionId?: string;
  stream?: boolean; // Added stream parameter
}

export interface RagflowChatResponsePayload {
  answer: string;
  sessionId: string;
}