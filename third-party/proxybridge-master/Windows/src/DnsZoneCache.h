#ifndef PROXYBRIDGE_DNS_ZONE_CACHE_H
#define PROXYBRIDGE_DNS_ZONE_CACHE_H

// DNS answers are observed before they are delivered to applications. Keep a
// bounded IP -> queried-domain cache so a zone rule can be evaluated when the
// first connection packet arrives. Entries expire with the DNS answer's TTL.
#define DNS_ZONE_BUCKETS 1024
#define DNS_ZONE_WAYS 8
#define DNS_ZONE_NAME_LENGTH 254

typedef struct {
    UINT32 ip;
    char domain[DNS_ZONE_NAME_LENGTH];
    ULONGLONG expires_at;
} DNS_ZONE_ENTRY;

static DNS_ZONE_ENTRY dns_zone_entries[DNS_ZONE_BUCKETS][DNS_ZONE_WAYS];
static SRWLOCK dns_zone_lock = SRWLOCK_INIT;

static UINT16 dns_zone_read_u16(const unsigned char *p)
{
    return (UINT16)(((UINT16)p[0] << 8) | p[1]);
}

static UINT32 dns_zone_read_u32(const unsigned char *p)
{
    return ((UINT32)p[0] << 24) | ((UINT32)p[1] << 16) |
        ((UINT32)p[2] << 8) | (UINT32)p[3];
}

static UINT32 dns_zone_bucket(UINT32 ip)
{
    return (ip * 2654435761u) & (DNS_ZONE_BUCKETS - 1);
}

static void dns_zone_store(UINT32 ip, const char *domain, UINT32 ttl)
{
    if (ip == 0 || domain[0] == '\0' || ttl == 0)
        return;

    if (ttl > 3600)
        ttl = 3600;
    ULONGLONG now = GetTickCount64();
    DNS_ZONE_ENTRY *bucket = dns_zone_entries[dns_zone_bucket(ip)];
    DNS_ZONE_ENTRY *slot = NULL;

    AcquireSRWLockExclusive(&dns_zone_lock);
    for (int i = 0; i < DNS_ZONE_WAYS; i++)
    {
        if (bucket[i].ip == ip && strcmp(bucket[i].domain, domain) == 0)
        {
            slot = &bucket[i];
            break;
        }
        if (slot == NULL && bucket[i].expires_at <= now)
            slot = &bucket[i];
    }
    if (slot == NULL)
    {
        slot = &bucket[0];
        for (int i = 1; i < DNS_ZONE_WAYS; i++)
            if (bucket[i].expires_at < slot->expires_at)
                slot = &bucket[i];
    }
    slot->ip = ip;
    strcpy_s(slot->domain, sizeof(slot->domain), domain);
    slot->expires_at = now + (ULONGLONG)ttl * 1000;
    ReleaseSRWLockExclusive(&dns_zone_lock);
}

static BOOL dns_zone_matches(UINT32 ip, const char *zone)
{
    size_t zone_len = strlen(zone);
    if (zone_len == 0)
        return FALSE;

    ULONGLONG now = GetTickCount64();
    DNS_ZONE_ENTRY *bucket = dns_zone_entries[dns_zone_bucket(ip)];
    BOOL matched = FALSE;
    AcquireSRWLockShared(&dns_zone_lock);
    for (int i = 0; i < DNS_ZONE_WAYS; i++)
    {
        DNS_ZONE_ENTRY *entry = &bucket[i];
        if (entry->ip != ip || entry->expires_at <= now)
            continue;
        size_t name_len = strlen(entry->domain);
        if (name_len > zone_len + 1 &&
            entry->domain[name_len - zone_len - 1] == '.' &&
            _stricmp(entry->domain + name_len - zone_len, zone) == 0)
        {
            matched = TRUE;
            break;
        }
    }
    ReleaseSRWLockShared(&dns_zone_lock);
    return matched;
}

// Decode a DNS name including compression pointers, keeping the offset after
// the original name separate from the location followed by a pointer.
static BOOL dns_zone_read_name(const unsigned char *data, size_t len,
    size_t *offset, char *name, size_t name_size)
{
    size_t cursor = *offset, next = cursor, used = 0;
    BOOL jumped = FALSE;
    for (int hops = 0; hops < 128; hops++)
    {
        if (cursor >= len)
            return FALSE;
        unsigned char label_len = data[cursor++];
        if ((label_len & 0xc0) == 0xc0)
        {
            if (cursor >= len)
                return FALSE;
            size_t pointer = ((size_t)(label_len & 0x3f) << 8) | data[cursor++];
            if (pointer >= len)
                return FALSE;
            if (!jumped)
                next = cursor;
            jumped = TRUE;
            cursor = pointer;
            continue;
        }
        if (label_len & 0xc0)
            return FALSE;
        if (label_len == 0)
        {
            if (!jumped)
                next = cursor;
            if (used == 0)
                return FALSE;
            name[used] = '\0';
            *offset = next;
            return TRUE;
        }
        if (cursor + label_len > len || used + label_len + 1 > name_size)
            return FALSE;
        if (used != 0)
            name[used++] = '.';
        for (unsigned int i = 0; i < label_len; i++)
        {
            unsigned char c = data[cursor++];
            if (c < 33 || c > 126)
                return FALSE;
            name[used++] = (c >= 'A' && c <= 'Z') ? (char)(c + 32) : (char)c;
        }
    }
    return FALSE;
}

static void dns_zone_observe_response(const unsigned char *data, size_t len)
{
    if (data == NULL || len < 12 || !(data[2] & 0x80) || (data[3] & 0x0f) != 0 ||
        dns_zone_read_u16(data + 4) != 1)
        return;

    char queried_name[DNS_ZONE_NAME_LENGTH];
    size_t offset = 12;
    if (!dns_zone_read_name(data, len, &offset, queried_name, sizeof(queried_name)) ||
        offset + 4 > len || dns_zone_read_u16(data + offset + 2) != 1)
        return;
    offset += 4;

    UINT16 answer_count = dns_zone_read_u16(data + 6);
    if (answer_count > 128)
        answer_count = 128;
    for (UINT16 i = 0; i < answer_count; i++)
    {
        char answer_name[DNS_ZONE_NAME_LENGTH];
        if (!dns_zone_read_name(data, len, &offset, answer_name, sizeof(answer_name)) ||
            offset + 10 > len)
            return;
        UINT16 type = dns_zone_read_u16(data + offset);
        UINT16 record_class = dns_zone_read_u16(data + offset + 2);
        UINT32 ttl = dns_zone_read_u32(data + offset + 4);
        UINT16 record_len = dns_zone_read_u16(data + offset + 8);
        offset += 10;
        if (offset + record_len > len)
            return;
        if (type == 1 && record_class == 1 && record_len == 4)
        {
            UINT32 ip;
            memcpy(&ip, data + offset, sizeof(ip));
            dns_zone_store(ip, queried_name, ttl);
        }
        offset += record_len;
    }
}

#endif
