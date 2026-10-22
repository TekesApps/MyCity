"""Read macOS xattrs without following links; Xcode Python lacks os.listxattr."""
import ctypes,ctypes.util,os
lib=ctypes.CDLL(ctypes.util.find_library('c'),use_errno=True)
lib.listxattr.argtypes=[ctypes.c_char_p,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_int];lib.listxattr.restype=ctypes.c_ssize_t
lib.getxattr.argtypes=[ctypes.c_char_p,ctypes.c_char_p,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_uint32,ctypes.c_int];lib.getxattr.restype=ctypes.c_ssize_t
lib.setxattr.argtypes=[ctypes.c_char_p,ctypes.c_char_p,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_uint32,ctypes.c_int];lib.setxattr.restype=ctypes.c_int
lib.removexattr.argtypes=[ctypes.c_char_p,ctypes.c_char_p,ctypes.c_int];lib.removexattr.restype=ctypes.c_int
NOFOLLOW=1
def check(n,p):
 if n<0:raise OSError(ctypes.get_errno(),os.strerror(ctypes.get_errno()),str(p))
 return n
def listxattr(p,follow_symlinks=False):
 flag=0 if follow_symlinks else NOFOLLOW;p=os.fsencode(p);n=check(lib.listxattr(p,None,0,flag),p);b=ctypes.create_string_buffer(n);check(lib.listxattr(p,b,n,flag),p);return [os.fsdecode(x) for x in b.raw.split(b'\0') if x]
def getxattr(p,n,follow_symlinks=False):
 flag=0 if follow_symlinks else NOFOLLOW;p=os.fsencode(p);n=os.fsencode(n);k=check(lib.getxattr(p,n,None,0,0,flag),p);b=ctypes.create_string_buffer(k);check(lib.getxattr(p,n,b,k,0,flag),p);return b.raw
def setxattr(p,n,b,follow_symlinks=False):check(lib.setxattr(os.fsencode(p),os.fsencode(n),b,len(b),0,0 if follow_symlinks else NOFOLLOW),p)
def removexattr(p,n,follow_symlinks=False):check(lib.removexattr(os.fsencode(p),os.fsencode(n),0 if follow_symlinks else NOFOLLOW),p)
