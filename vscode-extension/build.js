// Process the claytip.tmLanguage.json to replace all the {{values}} with their replacement from replacements.props

const fs = require('fs');

const src = "src";
const out = "out";

function loadReplacements() {
  let content = fs.readFileSync(`${src}/syntaxes/replacements.props`, 'utf8');

  let lines = content.split("\n");

  let replacements = {};

  for (const line of lines) {
    let parts = line.split("=");
    if (parts.length == 2) {
      replacements[parts[0]] = parts[1];
    } else {
      throw new Error(`Invalid line: ${line}`);
    }
  }

  return replacements;
}

function processTemplate() {
  let content = fs.readFileSync(`${src}/syntaxes/claytip.tmLanguage.template.json`, 'utf8');

  const replacements = loadReplacements();
  let keys = Object.keys(replacements);

  for (const key of keys) {
    content = content.replace(/{{${key}}}/g, replacements[key]);
  }

  fs.writeFileSync(`${out}/syntaxes/claytip.tmLanguage.json`, content);
}

if (!fs.existsSync(out)) {
  fs.mkdirSync(out);
}

if (!fs.existsSync(`${out}/syntaxes`)) {
  fs.mkdirSync(`${out}/syntaxes`);
}

fs.copyFileSync(`${src}/package.json`, `${out}/package.json`);
fs.copyFileSync(`${src}/language-configuration.json`, `${out}/language-configuration.json`);
fs.copyFileSync(`${src}/syntaxes/clay.markdown.codeblock.json`, `${out}/syntaxes/clay.markdown.codeblock.json`);

processTemplate()
