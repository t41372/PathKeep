# Builds search-index-v1.idx, an encrypted search index file made without PathKeep's code, so
# tests/search_dir.rs can check that the Rust implementation reads the format exactly as
# documented in src/search_dir/format.rs. Run from this directory: python3 make_search_index_vector.py
# Needs the `cryptography` package.
import hashlib, hmac
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

def hkdf(ikm, salt, info):
    prk = hmac.new(salt or b"\0" * 32, ikm, hashlib.sha256).digest()
    return hmac.new(prk, info + b"\x01", hashlib.sha256).digest()

data_key = bytes([0x11] * 32)
index_key = hkdf(data_key, None, b"pathkeep/v1/search-index")
key_id = hkdf(index_key, None, b"pathkeep/v1/search-index-key-id")[:16]
salt = bytes(range(0x80, 0xA0))
file_key = hkdf(index_key, salt, b"pathkeep/v1/search-index-file")
header = b"PKSI" + bytes([1]) + key_id + salt
name = b"vector.idx"
plaintext = bytes(i % 251 for i in range(65536 + 3))
blocks = [plaintext[i:i + 65536] for i in range(0, len(plaintext), 65536)]
out = bytearray(header)
for index, block in enumerate(blocks):
    final = index == len(blocks) - 1
    aad = header + index.to_bytes(8, "little") + len(block).to_bytes(4, "little") + bytes([final]) + name
    nonce = index.to_bytes(8, "little") + bytes(4)
    out += AESGCM(file_key).encrypt(nonce, block, aad)
open("search-index-v1.idx", "wb").write(out)
print(len(out), hashlib.sha256(out).hexdigest())
