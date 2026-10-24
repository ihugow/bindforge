# Autorizações de dispositivos no Linux

O BindForge permite consultar, conceder e remover autorizações por interface física,
sem executar a interface ou o daemon como root. O RPM inclui um helper administrativo
fixo e uma política polkit. Instalar o RPM prepara o mecanismo, mas não concede
acesso a todos os teclados/mouses nem ativa captura.

## Fluxo pela interface

1. Cadastre e salve o dispositivo em Dispositivos.
2. Abra o cadastro e use **Gerenciar acesso**, ou use o botão correspondente em
   **Início → Remapeamento do mouse/teclado**.
3. Confira o nome da interface detectada, fabricante/produto, escopo e estado.
4. Clique em **Conceder acesso** e autentique-se pela janela do sistema.
5. Ative o remapeamento separadamente quando desejar.

A tela mostra leitura da entrada, acesso à saída virtual e presença de uma regra
criada pelo BindForge. Permissões manuais, regras antigas e permissões de outros
programas podem permitir acesso sem uma autorização gerenciada; nesse caso, não
há botão para remover essas permissões externas.

**Remover autorização** pausa os slots do daemon associados à interface antes da
autenticação. Cancelar a autenticação mantém a regra e deixa a captura pausada.
A operação remove somente as regras gerenciadas pelo BindForge e grava uma revogação
persistente para a identidade/interface. O daemon recusa novas ativações enquanto
essa revogação existir, mesmo que a entrada ainda esteja legível por ACL residual
ou regra externa. Apenas uma nova concessão administrativa bem-sucedida limpa a
revogação. A tela indica **Revogada · ativação bloqueada**.

Reconectar o dispositivo serve para encerrar ACLs residuais do sistema: não é
necessário reconectar para bloquear o BindForge. Remover uma regra udev não invalida
automaticamente descritores abertos nem todas as permissões existentes. Regras externas
continuam podendo conceder acesso ao sistema/outros aplicativos após reconexão. Se não houver daemon em execução, a regra também pode
ser removida pela tela do cadastro.

Desinstalar o pacote não remove automaticamente as autorizações criadas em /etc.
Antes de desinstalar, remova as autorizações pela interface e reconecte os dispositivos.
Revogações já gravadas também permanecem para preservar a escolha após reinstalação.

O dispositivo precisa estar conectado e corretamente vinculado para gerenciar sua
autorização nesta versão. O vínculo do cadastro ainda é local à conexão; após
reconectar, pode ser necessário selecionar a nova interface. A autorização udev
não depende do número eventN.

## Escopo das regras

O helper reenumera os dispositivos físicos via udev; não aceita texto de regra,
caminho de arquivo de configuração, usuário de destino ou comando arbitrário.
A escolha é limitada a mouse/teclado físicos reconhecidos e ao tipo cadastrado.
Interfaces virtuais, saídas do BindForge e identidades ambíguas são recusadas.

- USB: fabricante, produto, número da interface, tipo de entrada e número de série
  individual quando disponível. Um identificador uniq físico pode ser usado quando
  existe no input parent, como no Logitech deste ambiente.
- Sem série/uniq: porta ID_PATH, fabricante/produto e interface. Mover o dispositivo
  para outra porta requer nova autorização; outro exemplar equivalente na mesma
  porta/interface também pode corresponder. A UI informa esse escopo.
- Bluetooth: IDs do input parent, uniq e phys, evitando confundir suas interfaces.
- Integrados: IDs do input parent e ID_PATH. Metadados incompletos ou caracteres de
  glob/escape não são aceitos para gerar uma regra mais ampla como fallback.

Uma revogação fica em um arquivo `.revoked` com a mesma identidade/hash da regra,
conteúdo canônico e propriedade administrativa. Ele não é uma regra udev. O daemon
recebe uma política de acesso da aplicação, verifica-a antes de abrir/grab e consulta
a revogação periodicamente durante a captura (intervalo de 100 ms). Uma falha da
política encerra o worker pelo mesmo caminho que libera teclas, saída e grab.
A interface também confirma a pausa após a autenticação para cobrir uma ativação
feita enquanto a janela do polkit estava aberta. Entrada sem histórico de revogação
continua compatível com permissões externas já existentes.

Uma regra identifica exatamente uma interface no inventário atual. Ela tem nome
`72-bindforge-access-<sha256>.rules` em `/etc/udev/rules.d/`, antes do processamento
uaccess de `73-seat-late.rules`. Cada concessão também prepara uma regra compartilhada
restrita ao nó misc/uinput: `72-bindforge-uinput.rules`. Essa regra só é removida
quando a última autorização gerenciada é removida, e a revogação nunca cria novas
permissões compartilhadas. O módulo uinput é carregado se necessário.

uaccess concede acesso a programas do usuário da sessão local ativa; não é uma
permissão exclusiva de um executável. A interface explica tanto o acesso à entrada
quanto o acesso à saída virtual antes da autenticação.

## Separação de privilégios

- Interface/daemon continuam como usuário comum.
- `bindforge-access` é uma biblioteca de inspeção de metadados/geração de regras,
  com SHA-256 para identificar o conteúdo canônico.
- O executável administrativo é instalado em
  `/usr/libexec/bindforge/bindforge-device-access`, sem setuid.
- As ações `org.bindforge.device-access.grant` e `.revoke` usam auth_admin para
  sessão ativa e negam as demais sessões. Não há autorização administrativa em cache
  pela política distribuída. Cada ação fixa o caminho e o primeiro argumento do
  helper via anotações pkexec. A política fica em
  `/usr/share/polkit-1/actions/org.bindforge.device-access.policy`.
- O shell Tauri valida a janela, carrega o cadastro salvo e invoca apenas
  `/usr/bin/pkexec --disable-internal-agent` com argumentos separados. Não executa
  shell, sudo, binário de desenvolvimento, comandos do frontend ou arquivo arbitrário.
- O fingerprint consultado pela UI é exigido antes da operação e novamente pelo
  helper após autenticação e sob lock. Uma mudança de interface exige nova consulta.
- O helper revalida tipo/identidade no udev, rejeita execução sem root e usa apenas
  operações grant/revoke. Seus subprocessos são udevadm e, se necessário, modprobe
  uinput, com caminhos fixos, ambiente limpo e prazos limitados.
- Arquivos externos, links simbólicos/hardlinks, diretórios não administrativos e
  arquivos modificados ou graváveis por terceiros são preservados e retornam erro.
  Escrita atômica, lock não bloqueante e rollback protegem falhas parciais das regras
e do registro de revogação. Uma concessão com erro preserva uma revogação anterior.
  Se alguma ACL já tiver sido aplicada antes de uma falha, pode ser necessário
  reconectar; a mensagem informa essa limitação.

## Instalação e desenvolvimento

```bash
npm run package:fedora --prefix apps/desktop
```

O RPM inclui UI, daemon, helper, unidade e política. Para atualizar uma instalação
com a mesma versão de pacote, reinstale o RPM local e reabra a interface. O pacote
não instala a antiga regra manual específica do Aula F99.

Em desenvolvimento, o shell também usa somente o helper instalado em /usr/libexec.
Se ele ou sua política estiverem ausentes, a tela explica que o componente precisa
ser instalado e mantém a concessão desabilitada. A alternativa é instalar somente
os dois artefatos administrativos construídos, mantendo UI/daemon de desenvolvimento:

```bash
cargo build --release -p bindforge-access --locked
sudo install -D -o root -g root -m 0755 target/release/bindforge-device-access /usr/libexec/bindforge/bindforge-device-access
sudo install -D -o root -g root -m 0644 integrations/polkit/org.bindforge.device-access.policy /usr/share/polkit-1/actions/org.bindforge.device-access.policy
```

Isso instala o mecanismo, sem conceder permissões a nenhum periférico. O polkit
recarrega a política; a concessão acontece depois, pela interface.

## Verificação

Testes isolados cobrem escopo serial/porta/Bluetooth/integrado, rejeição de injeção
de regras/globs, compartilhamento uinput, revogação, links/arquivos externos e rollback.
Testes do shell cobrem resposta inválida, cancelamento e recusa. A fixture de navegador
`apps/desktop/tests/device-access-browser.html` exercita o fluxo sem autenticar ou
alterar permissões reais.

As regras derivadas do Logitech G PRO Wireless e do Aula F99 desta máquina passaram
em udevadm verify. A política passou na validação XML. Testes com autenticação real,
instalação administrativa e reconexão dependem da instalação do componente e da
autenticação do usuário; não foram simulados como sucesso nem executados via root.

Referências: [udev](https://www.freedesktop.org/software/systemd/man/latest/udev.html),
[polkit](https://polkit.pages.freedesktop.org/polkit/polkit.8.html) e documentação local
`pkexec(1)` sobre exec.path e exec.argv1.


### Atualização do bloqueio de revogação

Atualize helper, interface e daemon juntos e reinicie o serviço. Versões anteriores
removiam somente a regra e dependiam de reconexão para perder o ACL do sistema.
Uma revogação feita nessa versão antiga não possui o novo registro persistente;
para testar a correção, conceda acesso e remova novamente após atualizar.
