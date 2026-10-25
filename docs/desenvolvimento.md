# Desenvolvimento local do BindForge

Use este fluxo para acompanhar mudanças sem gerar e reinstalar o RPM.

## Apenas a interface

Encerre a interface instalada pelo menu da bandeja, em **Sair da interface**.
Fechar a janela apenas a oculta.

Em um terminal:

```bash
cd /home/hugostoso/Dev/Projects/Personal/bindforge/apps/desktop
npm run tauri dev
```

O comando inicia o Vite, compila o desktop e abre uma janela nativa do BindForge.
Deixe o terminal aberto. A primeira compilação pode demorar.

- CSS e React: as mudanças aparecem automaticamente ao salvar.
- Rust do desktop: o aplicativo recompila e reinicia automaticamente.
- Não é necessário executar `npm run build` nem reinstalar o RPM.
- O atalho do menu abre a versão instalada; use a janela aberta pelo comando.
- Encerre a sessão de desenvolvimento com `Ctrl+C` no terminal.

Se o daemon instalado estiver em execução, a interface de desenvolvimento se
conecta a ele. O daemon é separado da interface: sair da interface não para o
serviço.

Não é preciso executar `npm run dev` separadamente. Se a porta 1420 estiver
ocupada por outro Vite, encerre esse servidor antes de iniciar `npm run tauri dev`.

## Interface e daemon em desenvolvimento

Use dois terminais para manter os logs de cada processo visíveis.

### Terminal 1: daemon

Pare o serviço instalado antes de iniciar o daemon manualmente, pois os dois
usam o mesmo nome no D-Bus:

```bash
cd /home/hugostoso/Dev/Projects/Personal/bindforge
systemctl --user stop bindforge-daemon.service
cargo run -p bindforge-daemon --features kde-dbus -- --kde
```

Deixe esse terminal aberto. O Cargo compila o daemon e o executa com integração
KDE. Para aplicar mudanças no código do daemon, pressione `Ctrl+C` e execute
novamente o comando `cargo run` acima; ele recompila o necessário.

### Terminal 2: interface

```bash
cd /home/hugostoso/Dev/Projects/Personal/bindforge/apps/desktop
npm run tauri dev
```

A interface conecta ao daemon de desenvolvimento automaticamente. Se já estiver
aberta, reconecta quando o daemon iniciar.

Enquanto o daemon estiver rodando pelo terminal, controle sua execução nesse
terminal. O painel de serviço pode identificá-lo como iniciado externamente.

## Perfis e configurações

As versões de desenvolvimento e instalada usam os mesmos perfis e configurações
salvos. Alterações feitas em desenvolvimento também aparecem na versão instalada.

Iniciar o daemon não ativa automaticamente o remapeamento. Ative o dispositivo
na interface quando quiser testar.

## Voltar à versão instalada

Se você iniciou o daemon manualmente conforme este guia:

1. Pressione `Ctrl+C` no terminal da interface de desenvolvimento.
2. Pressione `Ctrl+C` no terminal do daemon de desenvolvimento.
3. Inicie novamente o serviço instalado:

```bash
systemctl --user start bindforge-daemon.service
```

Abra o BindForge pelo menu do sistema.

Esse retorno pressupõe que a unidade do serviço continua apontando para o daemon
instalado. Se você usou o painel de desenvolvimento para iniciar, reiniciar ou
habilitar o serviço, ele pode ter configurado a unidade para o binário de debug.
Nesse caso, use o painel de serviço da interface instalada para iniciar ou
reiniciar o serviço; ele atualiza a unidade gerenciada para o executável do RPM.

## Diagnóstico

```bash
systemctl --user status bindforge-daemon.service
journalctl --user -u bindforge-daemon.service -n 40 --no-pager
```

Esses comandos mostram o serviço gerenciado pelo systemd. Os logs do daemon
iniciado com `cargo run` aparecem no terminal onde ele está rodando.
