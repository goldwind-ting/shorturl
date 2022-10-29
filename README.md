# A URL shortening service.

### The original URL is hashed by `CityHash` into a 32-bit number, and then append another 4 bits as sequence to resolve hash collision.

### Finnaly the 36-bit number is encoded to a six-character string by base64.