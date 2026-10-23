# Serviço do usuário e pacote Fedora/KDE

O painel **Início → Serviço do BindForge** gerencia somente
`bindforge-daemon.service` no systemd do usuário. Iniciar e reiniciar aguardam
o job do systemd terminar e o nome D-Bus pertencer ao PID da unidade. Parar
aguarda a unidade ficar inativa e o nome D-Bus desaparecer. Operações são
serializadas e têm prazo; falhas mostram estado, resultado e código de saída.
Um daemon iniciado fora da unidade é diagnosticado e não recebe sinais do app.

O serviço usa `Type=dbus`, a ponte KWin automática e o ciclo de sessão existente.
Não executa captura automaticamente. Fechar a UI mantém o serviço; encerrar a
sessão gráfica o encerra por `PartOf=graphical-session.target`. Após falha,
systemd tenta reiniciá-lo com limite de três tentativas em 30 segundos, sempre
com entrada pausada.

## Inicialização opcional

A opção **Iniciar serviço ao entrar no KDE** habilita a unidade para
`graphical-session.target`, sem iniciar a execução atual. Desativar não para
o daemon. A preferência é lida do systemd, sem cópia em outro arquivo.
Não há habilitação automática durante instalação, abertura do app ou build.

Ao iniciar, reiniciar ou habilitar, a UI prepara uma unidade do usuário em
`$XDG_CONFIG_HOME/systemd/user/`, com fallback para `~/.config/systemd/user/`.
Ela só substitui um arquivo regular com o marcador `# Managed by BindForge`;
arquivos personalizados e links são recusados. A gravação usa arquivo temporário
e rename. Caminhos ExecStart têm aspas e escapes de systemd, sem shell.

Em desenvolvimento, o executável é `target/debug/bindforge-daemon` no checkout:

```bash
cargo build -p bindforge-daemon --features kde-dbus --locked
```

O build de produção usa exclusivamente `/usr/libexec/bindforge/bindforge-daemon`.
Assim, passar do desenvolvimento ao RPM e iniciar pelo app atualiza a unidade
gerenciada para o executável instalado. Nenhum executável é procurado no PATH.

## Construir e instalar

```bash
cd apps/desktop
npm ci
npm run package:fedora
```

O script compila daemon e desktop em release no `target/` da raiz e gera um RPM
com ambos e a unidade em `/usr/lib/systemd/user/`. A distribuição atual é
Fedora 44 x86_64/KDE. O build não instala pacotes nem modifica a sessão.
O pacote não inclui regras udev e preserva os requisitos de acesso ao mouse
e `/dev/uinput` do backend existente.

Instale o RPM pelo gerenciador gráfico ou, na raiz do repositório:

```bash
sudo dnf install './target/release/bundle/rpm/BindForge-0.1.0-1.x86_64.rpm'
```

Abra **BindForge** no menu, inicie o serviço e ative o mouse desejado.
Se já houver um daemon manual, encerre-o primeiro no terminal de origem.

Para diagnóstico adicional, sem alterar o serviço:

```bash
systemctl --user status bindforge-daemon.service
journalctl --user -u bindforge-daemon.service -n 40 --no-pager
```

Antes de desinstalar, desmarque a inicialização no login e pare o serviço pela UI.
Perfis e cadastros não são apagados por essas operações.

## Validação

Os testes do shell usam barramento D-Bus privado e systemd simulado: cobrem
primeira instalação, habilitar sem iniciar, início/parada/reinício, espera pelo
job, desabilitar sem parar, proteção de daemon externo, preservação de unidade
personalizada e executável ausente. Não capturam hardware nem alteram o systemd
real. A fixture `apps/desktop/tests/service-browser.html` exercita o painel com
API em memória. Os registros finais ficam em [validation.md](validation.md).

A instalação do RPM, a inicialização no próximo login e um reinício completo
do computador ainda precisam ser confirmados na sessão real. A geração e inspeção
do RPM não comprovam esses cenários.
