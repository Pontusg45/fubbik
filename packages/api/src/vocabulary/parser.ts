export interface VocabEntry {
    word: string;
    category: string;
    expects: string[] | null;
}

export interface ParsedToken {
    text: string;
    category: string | null;
    position: { start: number; end: number };
}

export interface VocabularyWarning {
    position: { start: number; end: number };
    type: "unknown_word" | "unexpected_category" | "expects_not_satisfied";
    word: string;
    message: string;
}

export interface ParseResult {
    tokens: ParsedToken[];
    warnings: VocabularyWarning[];
}

interface ExtractedLiteral {
    text: string;
    start: number;
    end: number;
}

/**
 * Parse step text against a controlled vocabulary.
 *
 * Algorithm:
 * 1. Extract quoted strings and numbers, replace with placeholders
 * 2. Lowercase remaining text
 * 3. Sort vocabulary by word length descending (greedy longest match)
 * 4. Scan left-to-right matching longest vocab entry at each position
 * 5. Validate slot expectations between tokens
 * 6. Check dangling expects at end of sequence
 */
export function parseStepText(text: string, vocabulary: VocabEntry[]): ParseResult {
    if (text.trim() === "") {
        return { tokens: [], warnings: [] };
    }

    const tokens: ParsedToken[] = [];
    const warnings: VocabularyWarning[] = [];

    // Step 1: Extract quoted strings and numbers, track their positions
    const literals: ExtractedLiteral[] = [];
    // Replace quoted strings and numbers with placeholder spaces, preserving positions
    let processed = text;

    // Extract quoted strings (double and single quotes)
    const quoteRegex = /(["'])(?:(?!\1).)*\1/g;
    let match: RegExpExecArray | null;
    while ((match = quoteRegex.exec(text)) !== null) {
        literals.push({
            text: match[0],
            start: match.index,
            end: match.index + match[0].length
        });
    }

    // Extract standalone numbers (not inside quotes)
    const numberRegex = /\b\d+(?:\.\d+)?\b/g;
    while ((match = numberRegex.exec(text)) !== null) {
        // Check this number isn't inside a quoted string
        const inQuote = literals.some(l => match!.index >= l.start && match!.index < l.end);
        if (!inQuote) {
            literals.push({
                text: match[0],
                start: match.index,
                end: match.index + match[0].length
            });
        }
    }

    // Sort literals by start position
    literals.sort((a, b) => a.start - b.start);

    // Replace literals with spaces in the processed string
    for (let i = literals.length - 1; i >= 0; i--) {
        const lit = literals[i]!;
        processed = processed.substring(0, lit.start) + " ".repeat(lit.end - lit.start) + processed.substring(lit.end);
    }

    // Step 2: Lowercase remaining text
    const lowered = processed.toLowerCase();

    // Step 3: Sort vocabulary by word length descending
    const sortedVocab = [...vocabulary].sort((a, b) => b.word.length - a.word.length);
    const loweredVocabWords = sortedVocab.map(v => v.word.toLowerCase());

    // Build a set of positions consumed by literals or vocab matches
    const consumed = new Set<number>();

    // First, add literal tokens and mark their positions
    for (const lit of literals) {
        tokens.push({
            text: lit.text,
            category: "literal",
            position: { start: lit.start, end: lit.end }
        });
        for (let i = lit.start; i < lit.end; i++) {
            consumed.add(i);
        }
    }

    // Step 4: Scan left-to-right for vocab matches
    let pos = 0;
    while (pos < lowered.length) {
        // Skip consumed positions (literals)
        if (consumed.has(pos)) {
            pos++;
            continue;
        }

        // Skip whitespace
        if (/\s/.test(lowered[pos]!)) {
            pos++;
            continue;
        }

        let matched = false;

        // Try each vocab entry (longest first)
        for (let vi = 0; vi < sortedVocab.length; vi++) {
            const vocabWord = loweredVocabWords[vi]!;
            const vocabEntry = sortedVocab[vi]!;
            const end = pos + vocabWord.length;

            // Check if the word matches at this position
            if (end > lowered.length) continue;
            if (lowered.substring(pos, end) !== vocabWord) continue;

            // Check word boundary: next char must be whitespace, end, or consumed (literal)
            if (end < lowered.length && !/\s/.test(lowered[end]!) && !consumed.has(end)) continue;

            // Check start boundary: previous non-consumed char must be whitespace or start
            if (pos > 0 && !/\s/.test(lowered[pos - 1]!) && !consumed.has(pos - 1)) continue;

            tokens.push({
                text: text.substring(pos, end),
                category: vocabEntry.category,
                position: { start: pos, end }
            });
            for (let i = pos; i < end; i++) {
                consumed.add(i);
            }
            pos = end;
            matched = true;
            break;
        }

        if (!matched) {
            // Consume one whitespace-delimited word as unknown
            let wordEnd = pos;
            while (wordEnd < lowered.length && !/\s/.test(lowered[wordEnd]!) && !consumed.has(wordEnd)) {
                wordEnd++;
            }
            const unknownWord = text.substring(pos, wordEnd);
            tokens.push({
                text: unknownWord,
                category: null,
                position: { start: pos, end: wordEnd }
            });
            warnings.push({
                position: { start: pos, end: wordEnd },
                type: "unknown_word",
                word: unknownWord,
                message: `Unknown word: "${unknownWord}"`
            });
            for (let i = pos; i < wordEnd; i++) {
                consumed.add(i);
            }
            pos = wordEnd;
        }
    }

    // Sort tokens by position
    tokens.sort((a, b) => a.position.start - b.position.start);

    // Step 5: Validate slot expectations
    // Modifiers are transparent for expects validation
    let lastNonModifier: { entry: VocabEntry; token: ParsedToken } | null = null;

    for (const token of tokens) {
        // Literals satisfy any expects
        if (token.category === "literal") {
            if (lastNonModifier?.entry.expects) {
                // Literal satisfies expects — clear it
                lastNonModifier = null;
            }
            continue;
        }

        // Find the vocab entry for this token
        const vocabEntry = vocabulary.find(v => v.word.toLowerCase() === token.text.toLowerCase());

        if (!vocabEntry) {
            // Unknown word — check if previous expects is satisfied
            if (lastNonModifier?.entry.expects) {
                // Unknown words don't have a category, can't satisfy expects
                // But we already warned about unknown, so don't double-warn
            }
            continue;
        }

        // Modifiers are transparent
        if (vocabEntry.category === "modifier") {
            continue;
        }

        // Check if previous non-modifier had expects
        if (lastNonModifier?.entry.expects) {
            if (!lastNonModifier.entry.expects.includes(vocabEntry.category)) {
                warnings.push({
                    position: token.position,
                    type: "unexpected_category",
                    word: token.text,
                    message: `Expected ${lastNonModifier.entry.expects.join(" or ")} after "${lastNonModifier.token.text}", got ${vocabEntry.category} "${token.text}"`
                });
            }
        }

        lastNonModifier = { entry: vocabEntry, token };
    }

    // Step 6: End-of-sequence check
    if (lastNonModifier?.entry.expects) {
        const lastToken = lastNonModifier.token;
        warnings.push({
            position: lastToken.position,
            type: "expects_not_satisfied",
            word: lastToken.text,
            message: `"${lastToken.text}" expects ${lastNonModifier.entry.expects.join(" or ")} to follow, but step ends`
        });
    }

    return { tokens, warnings };
}
