#!/usr/bin/env node
// Sign a SHA256SUMS file with an Ed25519 private key and write SHA256SUMS.sig.
//
// Usage: node sign-checksums.mjs <sums-file>
// Env:   ATTOOL_UPDATE_SIGNING_PRIVATE_KEY = PKCS8 PEM (full multi-line block)
//
// Output: <sums-file>.sig (base64 of raw 64-byte Ed25519 signature, single line)

import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const sumsPath = process.argv[2];
if (!sumsPath) {
  console.error('usage: sign-checksums.mjs <sums-file>');
  process.exit(2);
}
const privatePem = process.env.ATTOOL_UPDATE_SIGNING_PRIVATE_KEY;
if (!privatePem) {
  console.error('ATTOOL_UPDATE_SIGNING_PRIVATE_KEY env var is not set');
  process.exit(2);
}

const bytes = fs.readFileSync(sumsPath);
const key = crypto.createPrivateKey({ key: privatePem, format: 'pem' });
const sig = crypto.sign(null, bytes, key);
const sigB64 = sig.toString('base64');
const sigPath = sumsPath + '.sig';
fs.writeFileSync(sigPath, sigB64 + '\n');
console.log(`wrote ${sigPath} (${sig.length} bytes → ${sigB64.length} base64 chars)`);
