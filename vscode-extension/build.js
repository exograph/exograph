// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

// Process the syntaxes/exograph.tmLanguage.template.json to replace all the {{values}} with their replacement from replacements.props

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
  let content = fs.readFileSync(`${src}/syntaxes/exograph.tmLanguage.template.json`, 'utf8');

  const replacements = loadReplacements();
  let keys = Object.keys(replacements);

  for (const key of keys) {
    content = content.replaceAll(`{{${key}}}`, replacements[key]);
  }

  fs.writeFileSync(`${out}/syntaxes/exograph.tmLanguage.json`, content);
}

if (!fs.existsSync(out)) {
  fs.mkdirSync(out);
}

if (!fs.existsSync(`${out}/syntaxes`)) {
  fs.mkdirSync(`${out}/syntaxes`);
}

fs.copyFileSync(`${src}/package.json`, `${out}/package.json`);
fs.copyFileSync(`${src}/language-configuration.json`, `${out}/language-configuration.json`);
fs.copyFileSync(`${src}/syntaxes/exo.markdown.codeblock.json`, `${out}/syntaxes/exo.markdown.codeblock.json`);

processTemplate()
