# redblock

A list of IP ranges (in CIDR format, newline-delimited) registered to regions that have enacted laws requiring age verification for online content deemed "harmful to minors."

These laws are often written in vague or overly broad terms, and in practice, they have been used or proposed to restrict access to LGBTQ+ content, sexual health information, and other constitutionally protected material.

**Note:** This list excludes regions where the relevant laws have been blocked by a court (i.e., under injunction) or have not yet gone into effect. This list gets updated every month or so.

## API
You can make calls to `https://redblock.dispherical.com/test?ip=[ipv4/ipv6]`

Privacy: I don't log your IP address and I have no need to. They're checked against ipset and then deleted from memory.

```js
{ "blocked": true } // if the user is in a restricted region
{ "blocked": false } // if the user is NOT in a restricted region
```

## Usage Instructions (ipset)

```bash
wget https://cdn.dispherical.com/redblock/list.txt
grep -v '^#' list.txt | grep -v '^[[:space:]]*$' > clean-list.txt

{
  echo "create blocked4 hash:net family inet hashsize 4096 maxelem 8000000"
  awk '/\./{print "add -exist blocked4", $0}' clean-list.txt
} > ipset-restore.txt

{
  echo "create blocked6 hash:net family inet6 hashsize 4096 maxelem 8000000"
  awk -F: '{if (NF>1) print "add -exist blocked6", $0}' clean-list.txt
} >> ipset-restore.txt

sudo ipset restore < ipset-restore.txt

sudo ipset test blocked[4/6] [ip]
# e.g. sudo ipset test blocked4 1.1.1.1
```

## Licensing

> Redblock uses IP geolocation data derived from DB-IP (db-ip.com), licensed under CC BY 4.0. This information is provided in a transformed and non-identifying format. The Redblock codebase and API are released into the public domain (CC0).

## Download
You may download the list from [https://cdn.dispherical.com/redblock/list.txt](https://cdn.dispherical.com/redblock/list.txt).

