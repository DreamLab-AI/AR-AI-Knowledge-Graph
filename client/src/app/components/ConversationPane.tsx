// client/src/app/components/ConversationPane.tsx
import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Button } from '@/features/design-system/components/Button';
import { Input } from '@/features/design-system/components/Input';
import { Send } from 'lucide-react';
import { apiService } from '@/services/api';
import { useSettingsStore } from '@/store/settingsStore'; // For auth check
import { nostrAuth } from '@/services/nostrAuthService'; // For getting token
import { createLogger, createErrorMetadata } from '@/utils/logger';
import MarkdownRenderer from '@/features/design-system/patterns/MarkdownRenderer'; // For rendering bot responses
import { RagflowChatRequestPayload, RagflowChatResponsePayload } from '@/types/ragflowTypes'; // Import DTOs
// import { shallow } from 'zustand/shallow'; // Not needed for this approach

const logger = createLogger('ConversationPane');

interface Message {
  id: string;
  sender: 'user' | 'bot';
  text: string;
}

const ConversationPane: React.FC = () => {
  const [messages, setMessages] = useState<Message[]>([]);
  const [currentInput, setCurrentInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);

  const authUser = useSettingsStore(state => state.user);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  useEffect(scrollToBottom, [messages]);

  const isPowerUser = authUser?.isPowerUser === true;

  const handleSendMessage = useCallback(async () => {
    if (!currentInput.trim() || isLoading || !isPowerUser) return;

    const userMessage: Message = { id: Date.now().toString() + '_user', sender: 'user', text: currentInput };
    setMessages(prev => [...prev, userMessage]);
    const question = currentInput;
    setCurrentInput('');
    setIsLoading(true);
    setError(null);

    try {
      const headers: Record<string, string> = {};
      if (nostrAuth.isAuthenticated()) {
        const nostrUser = nostrAuth.getCurrentUser();
        const token = nostrAuth.getSessionToken();
        if (nostrUser && token) {
          headers['X-Nostr-Pubkey'] = nostrUser.pubkey;
          headers['Authorization'] = `Bearer ${token}`;
        }
      }

      const payload: RagflowChatRequestPayload = { question, sessionId: sessionId ?? undefined, stream: false };
      const response: RagflowChatResponsePayload = await apiService.sendRagflowChatMessage(payload, headers);

      const botMessage: Message = { id: response.sessionId + '_bot', sender: 'bot', text: response.answer };
      setMessages(prev => [...prev, botMessage]);
      setSessionId(response.sessionId);
    } catch (err: any) {
      logger.error('Error sending RAGFlow message:', createErrorMetadata(err));
      const errorMessageText = err.message || 'Failed to get response.';
      setError(errorMessageText);
      const errorMessage: Message = { id: Date.now().toString() + '_error', sender: 'bot', text: `Error: ${errorMessageText}` };
      setMessages(prev => [...prev, errorMessage]);
    } finally {
      setIsLoading(false);
    }
  }, [currentInput, isLoading, sessionId, isPowerUser]);

  if (!isPowerUser && !authUser) { // Show loading or placeholder if authUser is not yet loaded
    return (
      <div className="flex flex-col items-center justify-center h-full p-4 bg-card text-card-foreground">
        <p className="text-muted-foreground">Loading user information...</p>
      </div>
    );
  }

  if (!isPowerUser) {
    return (
      <div className="flex flex-col items-center justify-center h-full p-4 bg-card text-card-foreground">
        <p className="text-lg font-semibold text-muted-foreground">RAGFlow Chat</p>
        <p className="text-sm text-muted-foreground mt-2">This chat feature is available for Power Users only.</p>
        <p className="text-xs text-muted-foreground mt-1">Please authenticate as a power user to enable RAGFlow chat.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full p-2 bg-background text-foreground">
      <div className="flex-grow overflow-y-auto mb-2 p-2 border border-border rounded custom-scrollbar space-y-3">
        {messages.map(msg => (
          <div key={msg.id} className={`flex ${msg.sender === 'user' ? 'justify-end' : 'justify-start'}`}>
            <div
              className={`max-w-[70%] p-3 rounded-lg shadow-sm text-sm break-words ${
                msg.sender === 'user'
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-secondary text-secondary-foreground'
              }`}
            >
              {msg.sender === 'bot' ? <MarkdownRenderer content={msg.text} className="prose prose-sm prose-invert max-w-none" /> : msg.text}
            </div>
          </div>
        ))}
        <div ref={messagesEndRef} />
      </div>
      {error && <p className="text-destructive text-xs mb-1 px-1">{error}</p>}
      <div className="flex items-center gap-2 p-1">
        <Input
          type="text"
          value={currentInput}
          onChange={(e) => setCurrentInput(e.target.value)}
          onKeyPress={(e) => e.key === 'Enter' && !isLoading && handleSendMessage()}
          placeholder="Ask RAGFlow..."
          className="flex-grow bg-input border-border placeholder-muted-foreground"
          disabled={isLoading || !isPowerUser}
          aria-label="Chat input"
        />
        <Button
            onClick={handleSendMessage}
            disabled={isLoading || !currentInput.trim() || !isPowerUser}
            size="icon"
            variant="ghost"
            aria-label="Send message"
            className="hover:bg-primary/10"
        >
          <Send size={18} className="text-primary" />
        </Button>
      </div>
    </div>
  );
};

export default ConversationPane;