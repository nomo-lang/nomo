pub(super) fn emit_crypto_helpers(out: &mut String) {
    out.push_str(
        r#"static uint32_t nomo_crypto_rotr32(uint32_t value, uint32_t amount) {
    return (value >> amount) | (value << (32U - amount));
}

static uint64_t nomo_crypto_rotr64(uint64_t value, uint64_t amount) {
    return (value >> amount) | (value << (64U - amount));
}

static nomo_string nomo_crypto_hex_string(const unsigned char *digest, size_t len) {
    static const char hex[] = "0123456789abcdef";
    char *out = (char *)malloc(len * 2 + 1);
    if (out == NULL) { nomo_panic("out of memory"); }
    for (size_t i = 0; i < len; i += 1) {
        out[i * 2] = hex[(digest[i] >> 4) & 0x0f];
        out[i * 2 + 1] = hex[digest[i] & 0x0f];
    }
    out[len * 2] = '\0';
    return nomo_string_owned(out);
}

static nomo_string nomo_crypto_sha256(nomo_string value) {
    static const uint32_t k[64] = {
        UINT32_C(0x428a2f98), UINT32_C(0x71374491), UINT32_C(0xb5c0fbcf), UINT32_C(0xe9b5dba5),
        UINT32_C(0x3956c25b), UINT32_C(0x59f111f1), UINT32_C(0x923f82a4), UINT32_C(0xab1c5ed5),
        UINT32_C(0xd807aa98), UINT32_C(0x12835b01), UINT32_C(0x243185be), UINT32_C(0x550c7dc3),
        UINT32_C(0x72be5d74), UINT32_C(0x80deb1fe), UINT32_C(0x9bdc06a7), UINT32_C(0xc19bf174),
        UINT32_C(0xe49b69c1), UINT32_C(0xefbe4786), UINT32_C(0x0fc19dc6), UINT32_C(0x240ca1cc),
        UINT32_C(0x2de92c6f), UINT32_C(0x4a7484aa), UINT32_C(0x5cb0a9dc), UINT32_C(0x76f988da),
        UINT32_C(0x983e5152), UINT32_C(0xa831c66d), UINT32_C(0xb00327c8), UINT32_C(0xbf597fc7),
        UINT32_C(0xc6e00bf3), UINT32_C(0xd5a79147), UINT32_C(0x06ca6351), UINT32_C(0x14292967),
        UINT32_C(0x27b70a85), UINT32_C(0x2e1b2138), UINT32_C(0x4d2c6dfc), UINT32_C(0x53380d13),
        UINT32_C(0x650a7354), UINT32_C(0x766a0abb), UINT32_C(0x81c2c92e), UINT32_C(0x92722c85),
        UINT32_C(0xa2bfe8a1), UINT32_C(0xa81a664b), UINT32_C(0xc24b8b70), UINT32_C(0xc76c51a3),
        UINT32_C(0xd192e819), UINT32_C(0xd6990624), UINT32_C(0xf40e3585), UINT32_C(0x106aa070),
        UINT32_C(0x19a4c116), UINT32_C(0x1e376c08), UINT32_C(0x2748774c), UINT32_C(0x34b0bcb5),
        UINT32_C(0x391c0cb3), UINT32_C(0x4ed8aa4a), UINT32_C(0x5b9cca4f), UINT32_C(0x682e6ff3),
        UINT32_C(0x748f82ee), UINT32_C(0x78a5636f), UINT32_C(0x84c87814), UINT32_C(0x8cc70208),
        UINT32_C(0x90befffa), UINT32_C(0xa4506ceb), UINT32_C(0xbef9a3f7), UINT32_C(0xc67178f2),
    };
    uint32_t h[8] = {
        UINT32_C(0x6a09e667), UINT32_C(0xbb67ae85), UINT32_C(0x3c6ef372), UINT32_C(0xa54ff53a),
        UINT32_C(0x510e527f), UINT32_C(0x9b05688c), UINT32_C(0x1f83d9ab), UINT32_C(0x5be0cd19),
    };
    size_t len = strlen(value.data);
    size_t padded_len = len + 1;
    while ((padded_len % 64) != 56) { padded_len += 1; }
    unsigned char *msg = (unsigned char *)calloc(padded_len + 8, 1);
    if (msg == NULL) { nomo_panic("out of memory"); }
    memcpy(msg, value.data, len);
    msg[len] = 0x80;
    uint64_t bit_len = (uint64_t)len * UINT64_C(8);
    for (size_t i = 0; i < 8; i += 1) {
        msg[padded_len + i] = (unsigned char)((bit_len >> (56 - 8 * i)) & 0xff);
    }
    for (size_t offset = 0; offset < padded_len + 8; offset += 64) {
        uint32_t w[64];
        for (size_t i = 0; i < 16; i += 1) {
            size_t j = offset + i * 4;
            w[i] = ((uint32_t)msg[j] << 24) | ((uint32_t)msg[j + 1] << 16) |
                ((uint32_t)msg[j + 2] << 8) | (uint32_t)msg[j + 3];
        }
        for (size_t i = 16; i < 64; i += 1) {
            uint32_t s0 = nomo_crypto_rotr32(w[i - 15], 7) ^ nomo_crypto_rotr32(w[i - 15], 18) ^ (w[i - 15] >> 3);
            uint32_t s1 = nomo_crypto_rotr32(w[i - 2], 17) ^ nomo_crypto_rotr32(w[i - 2], 19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16] + s0 + w[i - 7] + s1;
        }
        uint32_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hh = h[7];
        for (size_t i = 0; i < 64; i += 1) {
            uint32_t s1 = nomo_crypto_rotr32(e, 6) ^ nomo_crypto_rotr32(e, 11) ^ nomo_crypto_rotr32(e, 25);
            uint32_t ch = (e & f) ^ ((~e) & g);
            uint32_t temp1 = hh + s1 + ch + k[i] + w[i];
            uint32_t s0 = nomo_crypto_rotr32(a, 2) ^ nomo_crypto_rotr32(a, 13) ^ nomo_crypto_rotr32(a, 22);
            uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
            uint32_t temp2 = s0 + maj;
            hh = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
        }
        h[0] += a; h[1] += b; h[2] += c; h[3] += d; h[4] += e; h[5] += f; h[6] += g; h[7] += hh;
    }
    free(msg);
    unsigned char digest[32];
    for (size_t i = 0; i < 8; i += 1) {
        digest[i * 4] = (unsigned char)((h[i] >> 24) & 0xff);
        digest[i * 4 + 1] = (unsigned char)((h[i] >> 16) & 0xff);
        digest[i * 4 + 2] = (unsigned char)((h[i] >> 8) & 0xff);
        digest[i * 4 + 3] = (unsigned char)(h[i] & 0xff);
    }
    return nomo_crypto_hex_string(digest, 32);
}

static nomo_string nomo_crypto_sha512(nomo_string value) {
    static const uint64_t k[80] = {
        UINT64_C(0x428a2f98d728ae22), UINT64_C(0x7137449123ef65cd), UINT64_C(0xb5c0fbcfec4d3b2f), UINT64_C(0xe9b5dba58189dbbc),
        UINT64_C(0x3956c25bf348b538), UINT64_C(0x59f111f1b605d019), UINT64_C(0x923f82a4af194f9b), UINT64_C(0xab1c5ed5da6d8118),
        UINT64_C(0xd807aa98a3030242), UINT64_C(0x12835b0145706fbe), UINT64_C(0x243185be4ee4b28c), UINT64_C(0x550c7dc3d5ffb4e2),
        UINT64_C(0x72be5d74f27b896f), UINT64_C(0x80deb1fe3b1696b1), UINT64_C(0x9bdc06a725c71235), UINT64_C(0xc19bf174cf692694),
        UINT64_C(0xe49b69c19ef14ad2), UINT64_C(0xefbe4786384f25e3), UINT64_C(0x0fc19dc68b8cd5b5), UINT64_C(0x240ca1cc77ac9c65),
        UINT64_C(0x2de92c6f592b0275), UINT64_C(0x4a7484aa6ea6e483), UINT64_C(0x5cb0a9dcbd41fbd4), UINT64_C(0x76f988da831153b5),
        UINT64_C(0x983e5152ee66dfab), UINT64_C(0xa831c66d2db43210), UINT64_C(0xb00327c898fb213f), UINT64_C(0xbf597fc7beef0ee4),
        UINT64_C(0xc6e00bf33da88fc2), UINT64_C(0xd5a79147930aa725), UINT64_C(0x06ca6351e003826f), UINT64_C(0x142929670a0e6e70),
        UINT64_C(0x27b70a8546d22ffc), UINT64_C(0x2e1b21385c26c926), UINT64_C(0x4d2c6dfc5ac42aed), UINT64_C(0x53380d139d95b3df),
        UINT64_C(0x650a73548baf63de), UINT64_C(0x766a0abb3c77b2a8), UINT64_C(0x81c2c92e47edaee6), UINT64_C(0x92722c851482353b),
        UINT64_C(0xa2bfe8a14cf10364), UINT64_C(0xa81a664bbc423001), UINT64_C(0xc24b8b70d0f89791), UINT64_C(0xc76c51a30654be30),
        UINT64_C(0xd192e819d6ef5218), UINT64_C(0xd69906245565a910), UINT64_C(0xf40e35855771202a), UINT64_C(0x106aa07032bbd1b8),
        UINT64_C(0x19a4c116b8d2d0c8), UINT64_C(0x1e376c085141ab53), UINT64_C(0x2748774cdf8eeb99), UINT64_C(0x34b0bcb5e19b48a8),
        UINT64_C(0x391c0cb3c5c95a63), UINT64_C(0x4ed8aa4ae3418acb), UINT64_C(0x5b9cca4f7763e373), UINT64_C(0x682e6ff3d6b2b8a3),
        UINT64_C(0x748f82ee5defb2fc), UINT64_C(0x78a5636f43172f60), UINT64_C(0x84c87814a1f0ab72), UINT64_C(0x8cc702081a6439ec),
        UINT64_C(0x90befffa23631e28), UINT64_C(0xa4506cebde82bde9), UINT64_C(0xbef9a3f7b2c67915), UINT64_C(0xc67178f2e372532b),
        UINT64_C(0xca273eceea26619c), UINT64_C(0xd186b8c721c0c207), UINT64_C(0xeada7dd6cde0eb1e), UINT64_C(0xf57d4f7fee6ed178),
        UINT64_C(0x06f067aa72176fba), UINT64_C(0x0a637dc5a2c898a6), UINT64_C(0x113f9804bef90dae), UINT64_C(0x1b710b35131c471b),
        UINT64_C(0x28db77f523047d84), UINT64_C(0x32caab7b40c72493), UINT64_C(0x3c9ebe0a15c9bebc), UINT64_C(0x431d67c49c100d4c),
        UINT64_C(0x4cc5d4becb3e42b6), UINT64_C(0x597f299cfc657e2a), UINT64_C(0x5fcb6fab3ad6faec), UINT64_C(0x6c44198c4a475817),
    };
    uint64_t h[8] = {
        UINT64_C(0x6a09e667f3bcc908), UINT64_C(0xbb67ae8584caa73b), UINT64_C(0x3c6ef372fe94f82b), UINT64_C(0xa54ff53a5f1d36f1),
        UINT64_C(0x510e527fade682d1), UINT64_C(0x9b05688c2b3e6c1f), UINT64_C(0x1f83d9abfb41bd6b), UINT64_C(0x5be0cd19137e2179),
    };
    size_t len = strlen(value.data);
    size_t padded_len = len + 1;
    while ((padded_len % 128) != 112) { padded_len += 1; }
    unsigned char *msg = (unsigned char *)calloc(padded_len + 16, 1);
    if (msg == NULL) { nomo_panic("out of memory"); }
    memcpy(msg, value.data, len);
    msg[len] = 0x80;
    uint64_t bit_low = (uint64_t)len * UINT64_C(8);
    uint64_t bit_high = (uint64_t)len >> 61;
    for (size_t i = 0; i < 8; i += 1) {
        msg[padded_len + i] = (unsigned char)((bit_high >> (56 - 8 * i)) & 0xff);
        msg[padded_len + 8 + i] = (unsigned char)((bit_low >> (56 - 8 * i)) & 0xff);
    }
    for (size_t offset = 0; offset < padded_len + 16; offset += 128) {
        uint64_t w[80];
        for (size_t i = 0; i < 16; i += 1) {
            size_t j = offset + i * 8;
            w[i] = ((uint64_t)msg[j] << 56) | ((uint64_t)msg[j + 1] << 48) |
                ((uint64_t)msg[j + 2] << 40) | ((uint64_t)msg[j + 3] << 32) |
                ((uint64_t)msg[j + 4] << 24) | ((uint64_t)msg[j + 5] << 16) |
                ((uint64_t)msg[j + 6] << 8) | (uint64_t)msg[j + 7];
        }
        for (size_t i = 16; i < 80; i += 1) {
            uint64_t s0 = nomo_crypto_rotr64(w[i - 15], 1) ^ nomo_crypto_rotr64(w[i - 15], 8) ^ (w[i - 15] >> 7);
            uint64_t s1 = nomo_crypto_rotr64(w[i - 2], 19) ^ nomo_crypto_rotr64(w[i - 2], 61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16] + s0 + w[i - 7] + s1;
        }
        uint64_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hh = h[7];
        for (size_t i = 0; i < 80; i += 1) {
            uint64_t s1 = nomo_crypto_rotr64(e, 14) ^ nomo_crypto_rotr64(e, 18) ^ nomo_crypto_rotr64(e, 41);
            uint64_t ch = (e & f) ^ ((~e) & g);
            uint64_t temp1 = hh + s1 + ch + k[i] + w[i];
            uint64_t s0 = nomo_crypto_rotr64(a, 28) ^ nomo_crypto_rotr64(a, 34) ^ nomo_crypto_rotr64(a, 39);
            uint64_t maj = (a & b) ^ (a & c) ^ (b & c);
            uint64_t temp2 = s0 + maj;
            hh = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
        }
        h[0] += a; h[1] += b; h[2] += c; h[3] += d; h[4] += e; h[5] += f; h[6] += g; h[7] += hh;
    }
    free(msg);
    unsigned char digest[64];
    for (size_t i = 0; i < 8; i += 1) {
        digest[i * 8] = (unsigned char)((h[i] >> 56) & 0xff);
        digest[i * 8 + 1] = (unsigned char)((h[i] >> 48) & 0xff);
        digest[i * 8 + 2] = (unsigned char)((h[i] >> 40) & 0xff);
        digest[i * 8 + 3] = (unsigned char)((h[i] >> 32) & 0xff);
        digest[i * 8 + 4] = (unsigned char)((h[i] >> 24) & 0xff);
        digest[i * 8 + 5] = (unsigned char)((h[i] >> 16) & 0xff);
        digest[i * 8 + 6] = (unsigned char)((h[i] >> 8) & 0xff);
        digest[i * 8 + 7] = (unsigned char)(h[i] & 0xff);
    }
    return nomo_crypto_hex_string(digest, 64);
}

static nomo_array_u32 nomo_crypto_random_bytes(uint64_t count) {
    if (count > (uint64_t)SIZE_MAX) { nomo_panic("crypto.random_bytes count is too large"); }
    nomo_array_u32 out = nomo_array_u32_new();
#if defined(_WIN32)
    for (uint64_t i = 0; i < count; i += 1) {
        unsigned int value = 0;
        if (rand_s(&value) != 0) { nomo_panic("crypto.random_bytes failed"); }
        out = nomo_array_u32_push(out, (uint32_t)(value & 0xffU));
    }
#else
    FILE *file = fopen("/dev/urandom", "rb");
    if (file == NULL) { nomo_panic("crypto.random_bytes failed"); }
    for (uint64_t i = 0; i < count; i += 1) {
        unsigned char value = 0;
        if (fread(&value, 1, 1, file) != 1) {
            fclose(file);
            nomo_panic("crypto.random_bytes failed");
        }
        out = nomo_array_u32_push(out, (uint32_t)value);
    }
    fclose(file);
#endif
    return out;
}
"#,
    );
}
