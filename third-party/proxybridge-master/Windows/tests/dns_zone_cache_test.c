#include <winsock2.h>
#include <windows.h>
#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "../src/DnsZoneCache.h"

static void test_response(const char *name, unsigned char last_octet)
{
    unsigned char response[512] = {0};
    size_t pos = 12;
    response[2] = 0x81; // response, recursion desired
    response[3] = 0x80; // recursion available, no error
    response[5] = 1;    // one question
    response[7] = 1;    // one answer

    const char *label = name;
    while (*label)
    {
        const char *dot = strchr(label, '.');
        size_t len = dot ? (size_t)(dot - label) : strlen(label);
        assert(len > 0 && len <= 63);
        response[pos++] = (unsigned char)len;
        memcpy(response + pos, label, len);
        pos += len;
        label += len;
        if (*label == '.')
            label++;
    }
    response[pos++] = 0;
    response[pos++] = 0; response[pos++] = 1; // A
    response[pos++] = 0; response[pos++] = 1; // IN
    response[pos++] = 0xc0; response[pos++] = 12; // compressed answer owner
    response[pos++] = 0; response[pos++] = 1; // A
    response[pos++] = 0; response[pos++] = 1; // IN
    response[pos++] = 0; response[pos++] = 0;
    response[pos++] = 0; response[pos++] = 60; // TTL
    response[pos++] = 0; response[pos++] = 4;
    response[pos++] = 203; response[pos++] = 0;
    response[pos++] = 113; response[pos++] = last_octet;
    dns_zone_observe_response(response, pos);
}

static UINT32 test_ip(unsigned char last_octet)
{
    unsigned char bytes[] = {203, 0, 113, last_octet};
    UINT32 ip;
    memcpy(&ip, bytes, sizeof(ip));
    return ip;
}

int main(void)
{
    assert(!dns_zone_matches(test_ip(7), "ru"));
    test_response("WWW.Example.RU", 7);
    assert(dns_zone_matches(test_ip(7), "ru"));
    assert(dns_zone_matches(test_ip(7), "example.ru"));
    assert(!dns_zone_matches(test_ip(7), "u"));
    assert(!dns_zone_matches(test_ip(7), "com"));
    test_response("other.com", 8);
    assert(!dns_zone_matches(test_ip(8), "ru"));
    puts("DNS zone cache tests passed");
    return 0;
}
