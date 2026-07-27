const postcss = require('postcss');
const fs = require('fs');
for (const file of process.argv.slice(2)) {
  const root = postcss.parse(fs.readFileSync(file, 'utf8'), { from: file });
  let n = 0;
  root.walkComments((c) => { n++; c.remove(); });
  root.walk((node) => {
    if (node.raws && typeof node.raws.before === 'string') {
      node.raws.before = node.raws.before.replace(/\n{3,}/g, '\n\n');
    }
  });
  fs.writeFileSync(file, root.toString().replace(/\n{3,}/g, '\n\n').replace(/^\n+/, '') + '\n');
  console.log('stripped', file, `(${n} comments)`);
}
