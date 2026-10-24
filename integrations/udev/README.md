# Permissão opcional para o Aula F99

A regra `72-bindforge-aula-f99.rules` é opt-in e não é instalada pelo RPM nem
pelo daemon. Ela foi preparada para o receptor USB `3554:fa09`, interface `00`,
identificado neste ambiente como o teclado Aula F99. A combinação de IDs, número
da interface e `ID_INPUT_KEYBOARD=1` evita uma concessão genérica a todos os teclados.
O receptor não informa número de série individual: outra unidade com os mesmos IDs
e interface também corresponde à regra. Outros modos de conexão podem ter IDs
diferentes e precisam de verificação própria.

O tag `uaccess` concede acesso ao usuário da sessão local ativa por meio do udev /
logind. Não depende de um número eventN fixo, de uma porta USB específica, de um
nome de usuário ou de inclusão no grupo input. A regra precisa ser processada
antes de `73-seat-late.rules`, que aplica o ACL de sessão.

Instale a partir da raiz do projeto:

```bash
sudo install -m 0644 integrations/udev/72-bindforge-aula-f99.rules /etc/udev/rules.d/72-bindforge-aula-f99.rules
sudo udevadm control --reload-rules
```

Reconectar o receptor aplica a regra. Para aplicar ao nó atual sem reconectar,
confira o caminho com `udevadm info` e dispare apenas esse dispositivo. Na sessão
em que a regra foi preparada, o nó era `/dev/input/event3`:

```bash
sudo udevadm trigger --action=change /sys/class/input/event3
udevadm settle
getfacl /dev/input/event3
```

O ACL deve incluir o usuário da sessão ativa com leitura e escrita. O ACL manual
anterior pode continuar presente até a próxima recriação do nó; para confirmar a
persistência, reconecte o receptor e confira o ACL do novo nó. O vínculo do cadastro
BindForge ainda usa a identidade local à conexão: se ela mudar, atualize a lista
e selecione novamente a interface no cadastro do teclado.

Validação nesta máquina: `udevadm verify` passou; `udevadm test` com o diretório
adicional de regras produziu `uaccess` para a interface do teclado e enfileirou o
builtin correspondente de `73-seat-late.rules`. A simulação não executa os comandos
RUN. A instalação exige privilégio administrativo e não foi feita pelo agente,
pois `sudo -n` exige senha nesta sessão.

Para remover a configuração persistente:

```bash
sudo rm /etc/udev/rules.d/72-bindforge-aula-f99.rules
sudo udevadm control --reload-rules
```

Reconecte o receptor para recriar o nó sem a concessão desta regra.
