# Security Policy

## Status

**NO EXTERNAL AUDIT**

This is research code. Not production-ready.

## Reporting Issues

Email: security@truetrust.blockchain (research feedback only)

## Known Limitations

- Unoptimized STARK (timing attacks possible)
- No DOS protection
- No rate limiting
- Untested at scale
- May have race conditions

## Do Not Use For

- Production deployments
- Real value storage
- Critical applications

## Cryptography

- Falcon512: 128-bit classical, 64-bit quantum
- Kyber768: 192-bit classical, 96-bit quantum
- STARK (Goldilocks): 64-bit classical, 32-bit quantum
- Overall: Limited by weakest (64-bit classical)

## License

MIT - No warranty. Use at your own risk.
