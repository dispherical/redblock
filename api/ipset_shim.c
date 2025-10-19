#include <stdio.h>
#include <libipset/ipset.h>

static int quiet_error(struct ipset *session, void *p, int err_type,
                       const char *fmt, ...)
{
  (void)session;
  (void)p;
  (void)err_type;
  (void)fmt;
  return 0;
}

static int quiet_std(struct ipset *session, void *p)
{
  (void)session;
  (void)p;
  return 0;
}

static int quiet_out(struct ipset_session *session, void *p,
                     const char *fmt, ...)
{
  (void)session;
  (void)p;
  (void)fmt;
  return 0;
}

static int quiet_dbg(struct ipset *session, void *p,
                     const char *fmt, ...)
{
  (void)session;
  (void)p;
  (void)fmt;
  return 0;
}

int ipset_test_member(const char *setname, const char *elem)
{
  if (!setname || !elem)
    return -2;

  ipset_load_types();

  struct ipset *handle = ipset_init();
  if (!handle)
    return -1;

  ipset_custom_printf(handle, quiet_error, quiet_std, quiet_out, quiet_dbg);

  char *argv[] = {"ipset", "test", (char *)setname, (char *)elem, NULL};
  int argc = 4;

  int rc = ipset_parse_argv(handle, argc, argv);
  ipset_fini(handle);

  return rc == 0 ? 1 : 0;
}
