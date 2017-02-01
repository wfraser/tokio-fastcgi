#include <unistd.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>

const char PATH[] = "target/debug/examples/";

int main(int argc, char **argv) {
    // Set the umask to allow all to write to the socket the example makes.
    umask(0);

    char buf[1024];
    strcpy(buf, PATH);
    strncpy(buf + sizeof(PATH) - 1, argv[1], sizeof(buf) - sizeof(PATH));
    return execl(buf, argv[1], NULL);
}
