## UI components
- Before building any UI, check src/components/ui (shadcn) and src/components/ai-elements first. Reuse; never hand-roll an equivalent.
- Anything AI-related (chat, messages, streaming responses, reasoning, tool calls, prompt input, citations) MUST use AI Elements.
- If a needed component isn't installed, search for it via the shadcn MCP and install it rather than writing one from scratch.
- Custom components go in src/components/custom/ and use the same theme tokens (CSS variables).
- Never edit src/components/ai-elements/* for one-off styling; wrap them instead, so upstream updates stay easy.
