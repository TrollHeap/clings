/*
 * test.h — Minimal C unit-test harness for clings exercises.
 *
 * Usage in test_code (JSON field):
 *
 *   void test_something(void) {
 *       TEST_ASSERT_EQUAL_INT(42, my_function(6, 7));
 *   }
 *
 *   int main(void) {
 *       RUN_TEST(test_something);
 *       TEST_SUMMARY();
 *       return _clings_failures > 0 ? 1 : 0;
 *   }
 *
 * Output format (last line): "N Tests N Failures 0 Ignored"
 * This line is parsed by the clings runner to determine success.
 */

#ifndef CLINGS_TEST_H
#define CLINGS_TEST_H

#include <stdio.h>
#include <string.h>
#include <setjmp.h>

static int  _clings_tests    = 0;
static int  _clings_failures = 0;
static jmp_buf _clings_jmpbuf;
static const char *_clings_current_test = "";

/* ── Assertion helpers ─────────────────────────────────────────────────── */

#define TEST_FAIL_MSG(msg) \
    do { \
        printf("  FAIL  %s — %s\n", _clings_current_test, (msg)); \
        _clings_failures++; \
        longjmp(_clings_jmpbuf, 1); \
    } while (0)

#define TEST_ASSERT_TRUE(cond) \
    do { if (!(cond)) { TEST_FAIL_MSG(#cond " is false"); } } while (0)

#define TEST_ASSERT_FALSE(cond) \
    do { if (cond) { TEST_FAIL_MSG(#cond " is true"); } } while (0)

#define TEST_ASSERT_EQUAL_INT(expected, actual) \
    do { \
        int _e = (expected), _a = (actual); \
        if (_e != _a) { \
            char _buf[128]; \
            snprintf(_buf, sizeof(_buf), "expected %d but got %d", _e, _a); \
            TEST_FAIL_MSG(_buf); \
        } \
    } while (0)

#define TEST_ASSERT_EQUAL_STRING(expected, actual) \
    do { \
        const char *_e = (expected), *_a = (actual); \
        if (_a == NULL || strcmp(_e, _a) != 0) { \
            char _buf[256]; \
            snprintf(_buf, sizeof(_buf), "expected \"%s\" but got \"%s\"", \
                     _e, _a ? _a : "(null)"); \
            TEST_FAIL_MSG(_buf); \
        } \
    } while (0)

#define TEST_ASSERT_NULL(ptr) \
    do { if ((ptr) != NULL) { TEST_FAIL_MSG(#ptr " is not NULL"); } } while (0)

#define TEST_ASSERT_NOT_NULL(ptr) \
    do { if ((ptr) == NULL) { TEST_FAIL_MSG(#ptr " is NULL"); } } while (0)

/* ── Test runner ───────────────────────────────────────────────────────── */

#define RUN_TEST(fn) \
    do { \
        _clings_tests++; \
        _clings_current_test = #fn; \
        if (setjmp(_clings_jmpbuf) == 0) { \
            fn(); \
            printf("  OK    %s\n", #fn); \
        } \
    } while (0)

/* Print the summary line parsed by the clings runner. */
#define TEST_SUMMARY() \
    printf("\n%d Tests %d Failures 0 Ignored\n", _clings_tests, _clings_failures)

#endif /* CLINGS_TEST_H */
