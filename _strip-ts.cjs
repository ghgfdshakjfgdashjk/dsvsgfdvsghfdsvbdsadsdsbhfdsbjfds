const ts = require('typescript');
const fs = require('fs');

for (const file of process.argv.slice(2)) {
  const src = fs.readFileSync(file, 'utf8');
  const scanner = ts.createScanner(ts.ScriptTarget.Latest, false, ts.LanguageVariant.Standard, src);
  const ranges = [];
  let token;
  while ((token = scanner.scan()) !== ts.SyntaxKind.EndOfFileToken) {
    if (token === ts.SyntaxKind.SingleLineCommentTrivia || token === ts.SyntaxKind.MultiLineCommentTrivia) {
      ranges.push([scanner.getTokenPos(), scanner.getTextPos()]);
    }
    // Keep the scanner honest about regex vs division.
    if (token === ts.SyntaxKind.SlashToken || token === ts.SyntaxKind.SlashEqualsToken) {
      scanner.reScanSlashToken();
    }
  }
  let out = src;
  for (const [a, b] of ranges.reverse()) out = out.slice(0, a) + out.slice(b);
  out = out.split('\n').map((l) => l.replace(/\s+$/, ''));
  const kept = [];
  let blanks = 0;
  for (const l of out) {
    if (l.trim() === '') { if (++blanks > 1) continue; } else blanks = 0;
    kept.push(l);
  }
  while (kept.length && kept[0].trim() === '') kept.shift();
  while (kept.length && kept[kept.length - 1].trim() === '') kept.pop();
  fs.writeFileSync(file, kept.join('\n') + '\n');
  console.log('stripped', file, `(${ranges.length} comments)`);
}
