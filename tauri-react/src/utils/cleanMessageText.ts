export function cleanMessageText(text: string): string {
  if (!text) return '';
  let cleaned = text
    .replace(/<[^>]*tool[_\s-]?calls?\b[^>]*>/gi, '')
    .replace(/```(?:json|tool_call|toolcall)?\s*\{[\s\S]*?"(?:name|tool)"\s*:\s*"(?:read_file|write_file|patch_file|list_dir|grep|run_command|git_status|git_diff|git_log|git_branch)"[\s\S]*?```/gi, '');

  // Strip standalone or embedded JSON tool call objects with balanced braces
  const knownTools = ['read_file', 'write_file', 'patch_file', 'list_dir', 'grep', 'run_command', 'git_status', 'git_diff', 'git_log', 'git_branch'];
  let cur = 0;
  let result = '';
  while (cur < cleaned.length) {
    const openIdx = cleaned.indexOf('{', cur);
    if (openIdx === -1) {
      result += cleaned.slice(cur);
      break;
    }
    result += cleaned.slice(cur, openIdx);
    // Find balanced closing brace
    let depth = 0;
    let inString = false;
    let escape = false;
    let closeIdx = -1;
    for (let i = openIdx; i < cleaned.length; i++) {
      const ch = cleaned[i];
      if (escape) {
        escape = false;
        continue;
      }
      if (ch === '\\') {
        escape = true;
        continue;
      }
      if (ch === '"') {
        inString = !inString;
        continue;
      }
      if (inString) continue;
      if (ch === '{') depth++;
      else if (ch === '}') {
        depth--;
        if (depth === 0) {
          closeIdx = i;
          break;
        }
      }
    }

    if (closeIdx !== -1) {
      const candidate = cleaned.slice(openIdx, closeIdx + 1);
      try {
        const parsed = JSON.parse(candidate);
        const name = parsed?.name || parsed?.tool;
        if (name && knownTools.includes(name)) {
          // It's a tool call object! Skip it!
          cur = closeIdx + 1;
          continue;
        }
      } catch {
        // Not valid JSON, keep it
      }
      result += candidate;
      cur = closeIdx + 1;
    } else {
      result += cleaned.slice(openIdx);
      break;
    }
  }

  return result.trim();
}
