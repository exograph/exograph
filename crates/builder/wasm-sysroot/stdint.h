typedef signed char int8_t;
typedef short int16_t;
typedef long int32_t;
typedef long long int64_t;

typedef unsigned char uint8_t;
typedef unsigned short uint16_t;
typedef unsigned long uint32_t;
typedef unsigned long long uint64_t;

typedef unsigned long int	uintptr_t;

# define UINT8_C(c)	c
# define UINT32_C(c)	c ## U
# define UINT64_C(c)	c ## ULL

# define SIZE_MAX		(4294967295UL)
