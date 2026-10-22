#ifndef OSHWHUB_CORE_H
#define OSHWHUB_CORE_H
char *oshwhub_execute(const char *input);
void oshwhub_free(char *response);
void oshwhub_initialize(const char *directory);
#endif
