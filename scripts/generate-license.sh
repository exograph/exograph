#!/usr/bin/env bash

SCRIPT_DIR=`dirname -- "$0"`;

cargo about generate $SCRIPT_DIR/about.hbs | npx @html-to/text-cli --preserveNewlines > $SCRIPT_DIR/../THIRD-PARTY-LICENSES
npx generate-license-file --config $SCRIPT_DIR/generate-license-file-config.json --overwrite

sed -i 's/“/"/g' $SCRIPT_DIR/../THIRD-PARTY-LICENSES
sed -i 's/”/"/g' $SCRIPT_DIR/../THIRD-PARTY-LICENSES
sed -i 's/&lt;/</g' $SCRIPT_DIR/../THIRD-PARTY-LICENSES
sed -i 's/&gt;/>/g' $SCRIPT_DIR/../THIRD-PARTY-LICENSES

cat $SCRIPT_DIR/npm-licenses.txt >> $SCRIPT_DIR/../THIRD-PARTY-LICENSES

rm $SCRIPT_DIR/npm-licenses.txt
