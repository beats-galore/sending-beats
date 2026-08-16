#!/usr/bin/env node

const fs = require('fs');

const desc = process.argv[2] || 'migration';
const timestamp = new Date().toISOString().replace(/[-:T]/g, '').slice(0, 14);
const filename = `src-tauri/migrations/${timestamp}_${desc}.sql`;

console.log('Creating migration:', filename);

const content = `-- ${desc.replace(/_/g, ' ')}\n\n`;
fs.writeFileSync(filename, content);
