// Whisper annotates non-speech audio with bracketed/parenthesized tokens —
// "[Music]", "[BLANK_AUDIO]", "(applause)", "[ Silence ]", "*music*" — when the
// mic picks up music or ambient sound instead of a spoken command. These must
// never be sent to the LLM as if the user said them.

export function cleanTranscript(raw: string): string {
  return raw
    .replace(/[[(][^\])]*[\])]/g, " ") // strip [...] and (...) annotations
    .replace(/\*[^*]*\*/g, " ") // strip *music* style annotations
    .replace(/\s+/g, " ")
    .trim();
}

/** True only when real spoken words remain after stripping annotations. */
export function isMeaningfulSpeech(raw: string): boolean {
  return /[a-z0-9]{2,}/i.test(cleanTranscript(raw));
}
