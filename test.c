/* Luno compiled output */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
#include <math.h>
#include <unistd.h>

#ifdef _WIN32
#include <windows.h>
typedef CRITICAL_SECTION luno_mutex_t;
typedef CONDITION_VARIABLE luno_cond_t;
typedef HANDLE luno_thread_t;
#define luno_mutex_init(m) InitializeCriticalSection(m)
#define luno_mutex_lock(m) EnterCriticalSection(m)
#define luno_mutex_unlock(m) LeaveCriticalSection(m)
#define luno_mutex_destroy(m) DeleteCriticalSection(m)
#define luno_cond_init(c) InitializeConditionVariable(c)
#define luno_cond_wait(c,m) SleepConditionVariableCS((c),(m),INFINITE)
#define luno_cond_signal(c) WakeConditionVariable(c)
#define luno_cond_broadcast(c) WakeAllConditionVariable(c)
#define luno_cond_destroy(c) ((void)0)
#define luno_thread_create(t,f,a) (*(t)=CreateThread(NULL,0,(LPTHREAD_START_ROUTINE)(f),(a),0,NULL))
#define luno_thread_detach(t) CloseHandle(t)
#else
#include <pthread.h>
typedef pthread_mutex_t luno_mutex_t;
typedef pthread_cond_t luno_cond_t;
typedef pthread_t luno_thread_t;
#define luno_mutex_init(m) pthread_mutex_init(m,NULL)
#define luno_mutex_lock(m) pthread_mutex_lock(m)
#define luno_mutex_unlock(m) pthread_mutex_unlock(m)
#define luno_mutex_destroy(m) pthread_mutex_destroy(m)
#define luno_cond_init(c) pthread_cond_init(c,NULL)
#define luno_cond_wait(c,m) pthread_cond_wait((c),(m))
#define luno_cond_signal(c) pthread_cond_signal(c)
#define luno_cond_broadcast(c) pthread_cond_broadcast(c)
#define luno_cond_destroy(c) pthread_cond_destroy(c)
#define luno_thread_create(t,f,a) pthread_create((t),NULL,(f),(a))
#define luno_thread_detach(t) pthread_detach(t)
#endif

#ifdef _WIN32
#define luno_strdup(s) _strdup(s)
#else
#define luno_strdup(s) strdup(s)
#endif

typedef struct {
    char* data;
    int64_t len;
} LunoString;

LunoString luno_string_new(const char* s) {
    LunoString r;
    r.data = luno_strdup(s);
    r.len = (int64_t)strlen(s);
    return r;
}

void luno_string_free(LunoString* s) {
    free(s->data);
    s->data = NULL;
}

int64_t luno_print_string(LunoString s) {
    printf("%s\n", s.data);
    return 0;
}

int64_t luno_print_int(int64_t v) {
    printf("%lld\n", (long long)v);
    return 0;
}

int64_t luno_print_float(double v) {
    printf("%g\n", v);
    return 0;
}

int64_t luno_print_bool(bool v) {
    printf("%s\n", v ? "true" : "false");
    return 0;
}

typedef struct LunoFuture {
    void* result;
    int ready;
    luno_mutex_t mtx;
    luno_cond_t cv;
} LunoFuture;

LunoFuture* luno_future_new(void) {
    LunoFuture* f = (LunoFuture*)malloc(sizeof(LunoFuture));
    f->result = NULL;
    f->ready = 0;
    luno_mutex_init(&f->mtx);
    luno_cond_init(&f->cv);
    return f;
}

void luno_future_set(LunoFuture* f, void* val) {
    luno_mutex_lock(&f->mtx);
    f->result = val;
    f->ready = 1;
    luno_cond_broadcast(&f->cv);
    luno_mutex_unlock(&f->mtx);
}

void* luno_future_await(LunoFuture* f) {
    luno_mutex_lock(&f->mtx);
    while (!f->ready) {
        luno_cond_wait(&f->cv, &f->mtx);
    }
    luno_mutex_unlock(&f->mtx);
    void* result = f->result;
    luno_mutex_destroy(&f->mtx);
    luno_cond_destroy(&f->cv);
    free(f);
    return result;
}

typedef struct {
    void* (*fn)(void*);
    void* args;
    LunoFuture* future;
} LunoTask;

void* luno_task_run(void* arg) {
    LunoTask* task = (LunoTask*)arg;
    void* result = task->fn(task->args);
    luno_future_set(task->future, result);
    free(task);
    return NULL;
}

LunoFuture* luno_spawn(void* (*fn)(void*), void* args) {
    LunoFuture* fut = luno_future_new();
    LunoTask* task = (LunoTask*)malloc(sizeof(LunoTask));
    task->fn = fn;
    task->args = args;
    task->future = fut;
    luno_thread_t thread;
    luno_thread_create(&thread, luno_task_run, task);
    luno_thread_detach(thread);
    return fut;
}

typedef struct {
    void** buffer;
    int capacity;
    int count;
    int head;
    int tail;
    luno_mutex_t mtx;
    luno_cond_t not_full;
    luno_cond_t not_empty;
} LunoChan;

LunoChan* luno_chan_new(int capacity) {
    LunoChan* ch = (LunoChan*)malloc(sizeof(LunoChan));
    ch->buffer = (void**)malloc(sizeof(void*) * (size_t)capacity);
    ch->capacity = capacity;
    ch->count = 0;
    ch->head = 0;
    ch->tail = 0;
    luno_mutex_init(&ch->mtx);
    luno_cond_init(&ch->not_full);
    luno_cond_init(&ch->not_empty);
    return ch;
}

void luno_chan_send(LunoChan* ch, void* val) {
    luno_mutex_lock(&ch->mtx);
    while (ch->count >= ch->capacity) {
        luno_cond_wait(&ch->not_full, &ch->mtx);
    }
    ch->buffer[ch->tail] = val;
    ch->tail = (ch->tail + 1) % ch->capacity;
    ch->count++;
    luno_cond_signal(&ch->not_empty);
    luno_mutex_unlock(&ch->mtx);
}

void* luno_chan_recv(LunoChan* ch) {
    luno_mutex_lock(&ch->mtx);
    while (ch->count <= 0) {
        luno_cond_wait(&ch->not_empty, &ch->mtx);
    }
    void* val = ch->buffer[ch->head];
    ch->head = (ch->head + 1) % ch->capacity;
    ch->count--;
    luno_cond_signal(&ch->not_full);
    luno_mutex_unlock(&ch->mtx);
    return val;
}

void _luno_main();

void _luno_main() {
    luno_print_string(luno_string_new("Hello, world!"));
}

int main(int argc, char** argv) {
    (void)argc; (void)argv;
    _luno_main();
    return 0;
}
