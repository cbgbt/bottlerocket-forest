#!/usr/bin/env python3
"""Split markdown sentences onto separate lines for easier diff review."""

import re
import sys

def split_sentences(text):
    lines = text.split('\n')
    result = []
    
    for line in lines:
        # Preserve blank lines, headings, code blocks, lists, frontmatter
        if (not line.strip() or 
            line.startswith('#') or 
            line.startswith('```') or
            line.startswith('- ') or
            line.startswith('* ') or
            re.match(r'^\d+\.', line) or
            line.startswith('|') or
            line.startswith('---') or
            line.startswith('>')):
            result.append(line)
            continue
        
        # Split on sentence boundaries: . ! ? followed by space and capital letter
        # Preserve the punctuation with the sentence
        sentences = re.split(r'(?<=[.!?])\s+(?=[A-Z])', line)
        result.extend(sentences)
    
    return '\n'.join(result)

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: sentence-split.py <file.md> [--in-place]", file=sys.stderr)
        sys.exit(1)
    
    path = sys.argv[1]
    in_place = '--in-place' in sys.argv or '-i' in sys.argv
    
    with open(path) as f:
        content = f.read()
    
    result = split_sentences(content)
    
    if in_place:
        with open(path, 'w') as f:
            f.write(result)
    else:
        print(result)
