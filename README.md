# UPS
## Instalación
Para poder ejecutar este proyecto es necesario seguir una serie de pasos, así como instalar una serie de dependencias.
### Dependencias:
- llvm
- gcc
- qemu-system-x86
- rustup
### Instalación y configuración de cargo:
```sh
$ rustup install nightly
$ cargo install bootimage
$ rustup component add rust-src llvm-tools-preview
```
## Manual de uso
Una vez instaladas las dependencias y finalizada la inicialización podemos ejecutar el proyecto simplemente con el comando:
```sh
$ cargo +nightly run
```
Tras la instalación del proyecto veremos una ventana con el emulador ejecutándose, se inicializará el sistema y se nos permitirá utilizar la terminal.

En este punto podemos hacer varias cosas, a continuación se listan las más importantes:
- Listar los comandos disponibles con el comando ```help```.
![image](https://github.com/user-attachments/assets/64836f2e-ffce-4694-a603-a0e1aa3aeaea)
- Listar los componentes que se pueden depurar en esta versión utilizando el comando ```bk help```.
![image](https://github.com/user-attachments/assets/dd95c020-899f-49fe-852a-46cce42b2b3e)
- Activar los puntos de depuración utilizando el comando ```bk {componente}```
![image](https://github.com/user-attachments/assets/25f9dfd1-6669-4c46-af6f-fa6e1f2a8e0f)

Además de lo listado anteriormente podemos hacer gestiones simples como crear archivos o directorios, cambiar de directorio y comprobar la estructura del directorio en el que nos encontramos actualmente.

Todos estos comandos están listads en la llamada al comando help.
